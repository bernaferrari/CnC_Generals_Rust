//! W3D display system (port of W3DDisplay).
//!
//! Corresponds to C++ files:
//!   - GameEngineDevice/Include/W3DDevice/GameClient/W3DDisplay.h
//!   - GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp
//!
//! W3D implementation of the game display, responsible for creating and
//! maintaining the entire visual display: WGPU device/queue/surface init,
//! render pipeline orchestration, resolution management, FPS overlay,
//! screenshot capture, gamma correction, letterboxing, and clipping.

use crate::W3DDevice::GameClient::render_2d_pipeline::{DrawImageMode, Render2DPipeline};
use crate::W3DDevice::GameClient::wthree_d_asset_manager::WthreeDAssetManager;
use crate::W3DDevice::GameClient::wthree_d_dynamic_light::{
    LightType, W3DDynamicLight, MAX_LIGHTS,
};
use crate::W3DDevice::GameClient::wthree_d_scene::{
    CameraInfo, RenderInfo, W3D2DScene, W3DInterfaceScene, W3DScene,
};
use crate::W3DDevice::GameClient::wthree_d_shader_manager::WthreeDShaderManager;
use crate::W3DDevice::GameClient::wthree_d_terrain_visual::WthreeDTerrainVisual;
use crate::W3DDevice::GameClient::wthree_d_view::W3DView;
use anyhow::Result;
use cgmath::Vector3;
use parking_lot::RwLock;
use std::sync::{Arc, OnceLock};
use wgpu::{Device, Queue, Surface, SurfaceConfiguration};

// ---------------------------------------------------------------------------
// Constants (matching C++)
// ---------------------------------------------------------------------------

/// Default bit depth (C++ W3D_DISPLAY_DEFAULT_BIT_DEPTH)
const DEFAULT_BIT_DEPTH: u32 = 32;

/// Minimum display resolution X (C++ MIN_DISPLAY_RESOLUTION_X)
const MIN_DISPLAY_RESOLUTION_X: u32 = 800;

/// Minimum display resolution Y (C++ MIN_DISPLAY_RESOLUTOIN_Y)
const MIN_DISPLAY_RESOLUTION_Y: u32 = 600;

/// FPS history ring-buffer size (C++ FPS_HISTORY_SIZE)
const FPS_HISTORY_SIZE: usize = 30;

/// Maximum frame-time (seconds) accepted before ignoring spike (C++ MaximumFrameTimeCutoff)
const MAX_FRAME_TIME_CUTOFF: f64 = 0.5;

/// Frames per second (C++ standard: 30 FPS)
pub const LOGICFRAMES_PER_SECOND: u32 = 30;

/// Half-second skip for cumulative FPS measurement (C++ START_CUMU_FRAME)
const START_CUMU_FRAME: u32 = LOGICFRAMES_PER_SECOND / 2;

/// Letter-box fade duration in ms (C++ LETTER_BOX_FADE_TIME)
const LETTER_BOX_FADE_TIME_MS: f32 = 1000.0;

/// Time-of-day presets matching C++ TimeOfDay enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TimeOfDay {
    Morning = 0,
    Day = 1,
    Afternoon = 2,
    Evening = 3,
    Night = 4,
}

impl Default for TimeOfDay {
    fn default() -> Self {
        TimeOfDay::Day
    }
}

/// Terrain LOD levels matching C++ TerrainLOD enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TerrainLOD {
    Disable = 0,
    Max = 1,
    NoWater = 2,
    HalfClouds = 3,
    Automatic = 4,
    Min = 5,
}

impl Default for TerrainLOD {
    fn default() -> Self {
        TerrainLOD::Automatic
    }
}

/// Clipping region (matching C++ IRegion2D)
#[derive(Debug, Clone, Copy, Default)]
pub struct IRegion2D {
    pub lo_x: i32,
    pub lo_y: i32,
    pub hi_x: i32,
    pub hi_y: i32,
}

impl IRegion2D {
    pub fn new(lo_x: i32, lo_y: i32, hi_x: i32, hi_y: i32) -> Self {
        Self {
            lo_x,
            lo_y,
            hi_x,
            hi_y,
        }
    }
}

/// Light pulse creation parameters (matching C++ createLightPulse args)
#[derive(Debug, Clone)]
pub struct LightPulseParams {
    pub pos: (f32, f32, f32),
    pub color: (f32, f32, f32),
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub increase_frame_time: u32,
    pub decay_frame_time: u32,
}

// ---------------------------------------------------------------------------
// W3DDisplay
// ---------------------------------------------------------------------------

/// W3D implementation of the game display.
///
/// Corresponds to C++ `W3DDisplay : public Display`.
///
/// Owns the global scene, 2D scene, interface scene, asset manager, lights,
/// view, and all display state (resolution, gamma, clipping, letterbox).
pub struct W3DDisplay {
    // -- Initialization state --
    initialized: bool,
    windowed: bool,

    // -- Resolution / display --
    width: u32,
    height: u32,
    bit_depth: u32,
    gamma: f32,
    brightness: f32,
    contrast: f32,

    // -- Scenes --
    scene: Arc<RwLock<W3DScene>>,
    scene_2d: W3D2DScene,
    scene_3d_interface: W3DInterfaceScene,

    // -- View --
    view: RwLock<Option<W3DView>>,

    // -- Lights (C++ m_myLight[MAX_LIGHTS]) --
    global_lights: [Option<W3DDynamicLight>; MAX_LIGHTS],
    num_global_lights: usize,

    // -- Asset manager --
    asset_manager: Option<WthreeDAssetManager>,

    // -- Shader manager --
    shader_manager: Option<WthreeDShaderManager>,

    // -- Clipping (C++ m_clipRegion, m_isClippedEnabled) --
    clip_region: IRegion2D,
    clipping_enabled: bool,

    // -- FPS tracking (C++ m_averageFPS, fpsHistory[], updateAverageFPS()) --
    fps_history: [f64; FPS_HISTORY_SIZE],
    fps_history_offset: usize,
    fps_sample_count: usize,
    average_fps: f32,
    last_frame_time_nanos: i64,

    // -- Debug display (C++ m_displayStrings[]) --
    debug_display_enabled: bool,
    benchmark_timer: i32,

    // -- Letter-box (C++ m_letterBoxEnabled, m_letterBoxFadeLevel, etc.) --
    letter_box_enabled: bool,
    letter_box_fade_level: f32,
    letter_box_fade_start_time_ms: u64,

    // -- Screenshot / video capture --
    movie_capture_enabled: bool,

    // -- Time-of-day --
    time_of_day: TimeOfDay,

    // -- WGPU resources (held here for lifetime; actual GPU init happens in init()) --
    wgpu_device: Option<Arc<Device>>,
    wgpu_queue: Option<Arc<Queue>>,
    wgpu_surface: Option<Arc<Surface>>,
    wgpu_surface_config: Option<SurfaceConfiguration>,

    // -- 2D render pipeline (C++ m_2DRender / Render2DClass) --
    render_2d: Option<Render2DPipeline>,

    // -- Terrain visual (C++ W3DTerrainVisual / TheTerrainVisual) --
    terrain_visual: Option<WthreeDTerrainVisual>,
}

// ---------------------------------------------------------------------------
// Construction / global singleton
// ---------------------------------------------------------------------------

impl W3DDisplay {
    /// Create a new (uninitialized) display.
    pub fn new() -> Self {
        Self {
            initialized: false,
            windowed: false,
            width: MIN_DISPLAY_RESOLUTION_X,
            height: MIN_DISPLAY_RESOLUTION_Y,
            bit_depth: DEFAULT_BIT_DEPTH,
            gamma: 1.0,
            brightness: 0.0,
            contrast: 1.0,
            scene: Arc::new(RwLock::new(W3DScene::new())),
            scene_2d: W3D2DScene::new(),
            scene_3d_interface: W3DInterfaceScene::new(),
            view: RwLock::new(None),
            global_lights: Default::default(),
            num_global_lights: 0,
            asset_manager: None,
            shader_manager: None,
            clip_region: IRegion2D::default(),
            clipping_enabled: false,
            fps_history: [0.0; FPS_HISTORY_SIZE],
            fps_history_offset: 0,
            fps_sample_count: 0,
            average_fps: 30.0,
            last_frame_time_nanos: 0,
            debug_display_enabled: false,
            benchmark_timer: 0,
            letter_box_enabled: false,
            letter_box_fade_level: 0.0,
            letter_box_fade_start_time_ms: 0,
            movie_capture_enabled: false,
            time_of_day: TimeOfDay::Day,
            wgpu_device: None,
            wgpu_queue: None,
            wgpu_surface: None,
            wgpu_surface_config: None,
            render_2d: None,
            terrain_visual: None,
        }
    }

    // -- Global singleton (C++ static TheDisplay) --

    /// Return the global W3DDisplay singleton.
    pub fn global() -> Arc<RwLock<W3DDisplay>> {
        static DISPLAY: OnceLock<Arc<RwLock<W3DDisplay>>> = OnceLock::new();
        DISPLAY
            .get_or_init(|| Arc::new(RwLock::new(W3DDisplay::new())))
            .clone()
    }

    /// Convenience accessor for the global 3D scene.
    pub fn global_scene() -> Arc<RwLock<W3DScene>> {
        Self::global().read().scene()
    }
}

// ---------------------------------------------------------------------------
// init / reset  (C++ W3DDisplay::init, W3DDisplay::reset)
// ---------------------------------------------------------------------------

impl W3DDisplay {
    /// Initialize or re-initialize the W3D display system.
    ///
    /// C++ creates the W3D file system, math library, scenes, lights, asset
    /// manager, WGPU (WW3D) device, 2D renderer, debug display, etc.
    /// The Rust port defers actual WGPU device creation to `set_view()` which
    /// receives an already-created device/queue/surface.
    pub fn init(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(()); // re-entry guard (C++ @todo W3DDisplay needs RE-init logic)
        }

        // Create asset manager
        let mut asset_mgr = WthreeDAssetManager::new();
        asset_mgr.initialize()?;
        self.asset_manager = Some(asset_mgr);

        // Create shader manager
        let mut shader_mgr = WthreeDShaderManager::new();
        shader_mgr.init()?;
        self.shader_manager = Some(shader_mgr);

        // Create default global lights (C++ loop: m_myLight[i] = NEW_REF(LightClass, DIRECTIONAL))
        self.create_default_lights();

        // Set time-of-day to configure lights
        self.set_time_of_day(self.time_of_day);

        self.initialized = true;
        Ok(())
    }

    /// Reset the display between maps (C++ W3DDisplay::reset).
    ///
    /// Removes all render objects from the 3D scene and releases unused assets.
    pub fn reset(&mut self) {
        if let Some(ref mut scene) = self.scene.try_write() {
            scene.clear_render_objects();
        }
        self.clipping_enabled = false;

        // Release unused assets
        if let Some(ref mut asset_mgr) = self.asset_manager {
            asset_mgr.release_unused_assets();
        }
    }

    fn create_default_lights(&mut self) {
        // C++: for (lindex=0; lindex<TheGlobalData->m_numGlobalLights; lindex++)
        // Default to 1 directional light.
        self.num_global_lights = 1;
        let mut light = W3DDynamicLight::directional();
        light.set_diffuse(Vector3::new(1.0, 1.0, 0.9));
        light.set_ambient(Vector3::new(0.3, 0.3, 0.3));
        light.set_direction(Vector3::new(0.5, -1.0, 0.3));
        self.global_lights[0] = Some(light);

        // Register lights with the scene
        let mut scene = self.scene.write();
        for i in 0..self.num_global_lights {
            if let Some(ref light) = self.global_lights[i] {
                scene.set_global_light(light.clone(), i);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Resolution / display mode (C++ setDisplayMode, setWidth, setHeight)
// ---------------------------------------------------------------------------

impl W3DDisplay {
    /// Set the display resolution. Returns true if the mode change succeeded.
    ///
    /// C++ calls WW3D::Set_Device_Resolution and falls back to 16-bit if
    /// 32-bit fails. In WGPU, we reconfigure the surface.
    pub fn set_display_mode(
        &mut self,
        xres: u32,
        yres: u32,
        bitdepth: u32,
        windowed: bool,
    ) -> bool {
        if xres < MIN_DISPLAY_RESOLUTION_X || yres < MIN_DISPLAY_RESOLUTION_Y {
            return false;
        }

        self.width = xres;
        self.height = yres;
        self.bit_depth = bitdepth;
        self.windowed = windowed;

        // Reconfigure WGPU surface if available
        if let (Some(ref surface), Some(ref device), Some(ref mut config)) = (
            &self.wgpu_surface,
            &self.wgpu_device,
            &mut self.wgpu_surface_config,
        ) {
            config.width = xres;
            config.height = yres;
            surface.configure(device, config);
        }

        true
    }

    /// Get the current width.
    pub fn get_width(&self) -> u32 {
        self.width
    }

    /// Get the current height.
    pub fn get_height(&self) -> u32 {
        self.height
    }

    /// Get the current bit depth.
    pub fn get_bit_depth(&self) -> u32 {
        self.bit_depth
    }

    /// Check if running windowed.
    pub fn is_windowed(&self) -> bool {
        self.windowed
    }

    /// Set windowed mode flag.
    pub fn set_windowed(&mut self, windowed: bool) {
        self.windowed = windowed;
    }

    /// Set width and propagate to 2D coordinate range (C++ setWidth).
    pub fn set_width(&mut self, width: u32) {
        self.width = width;
        self.reconfigure_surface();
    }

    /// Set height and propagate to 2D coordinate range (C++ setHeight).
    pub fn set_height(&mut self, height: u32) {
        self.height = height;
        self.reconfigure_surface();
    }

    fn reconfigure_surface(&mut self) {
        if let (Some(ref surface), Some(ref device), Some(ref mut config)) = (
            &self.wgpu_surface,
            &self.wgpu_device,
            &mut self.wgpu_surface_config,
        ) {
            config.width = self.width;
            config.height = self.height;
            surface.configure(device, config);
        }
        if let Some(ref mut pipeline) = self.render_2d {
            pipeline.resize(self.width, self.height);
        }
    }

    /// Get number of display modes (C++ getDisplayModeCount).
    /// WGPU does not expose display mode enumeration; return a fixed set.
    pub fn get_display_mode_count(&self) -> i32 {
        // C++ iterates WW3D resolutions filtering for 4:3 >= 800x600 >= 24-bit.
        // WGPU doesn't expose this. Return common 4:3 modes.
        3
    }

    /// Get description of a display mode by index (C++ getDisplayModeDescription).
    pub fn get_display_mode_description(&self, index: i32) -> Option<(i32, i32, i32)> {
        let modes: [(i32, i32, i32); 3] = [(800, 600, 32), (1024, 768, 32), (1280, 1024, 32)];
        modes.get(index as usize).copied()
    }
}

// ---------------------------------------------------------------------------
// Gamma (C++ setGamma)
// ---------------------------------------------------------------------------

impl W3DDisplay {
    /// Set display gamma, brightness, and contrast.
    ///
    /// C++: `DX8Wrapper::Set_Gamma(gamma, bright, contrast, calibrate, false)`
    /// In WGPU, gamma is applied via a post-processing tonemapping pass or
    /// surface configuration. We store the values for later application.
    pub fn set_gamma(&mut self, gamma: f32, bright: f32, contrast: f32, _calibrate: bool) {
        if self.windowed {
            return; // C++: don't allow gamma change in windowed mode
        }
        self.gamma = gamma;
        self.brightness = bright;
        self.contrast = contrast;
    }

    /// Get current gamma value.
    pub fn get_gamma(&self) -> f32 {
        self.gamma
    }
}

// ---------------------------------------------------------------------------
// Time of day / lighting (C++ setTimeOfDay, createLightPulse)
// ---------------------------------------------------------------------------

impl W3DDisplay {
    /// Set the time of day, which reconfigures all global lights.
    ///
    /// C++ reads `TheGlobalData->m_terrainObjectsLighting[tod]` to set each
    /// light's ambient, diffuse, and position. We use simplified presets.
    pub fn set_time_of_day(&mut self, tod: TimeOfDay) {
        self.time_of_day = tod;

        let (ambient, diffuse, direction) = match tod {
            TimeOfDay::Morning => (
                Vector3::new(0.4, 0.35, 0.3),
                Vector3::new(1.0, 0.8, 0.5),
                Vector3::new(0.6, -1.0, 0.3),
            ),
            TimeOfDay::Day => (
                Vector3::new(0.3, 0.3, 0.3),
                Vector3::new(1.0, 1.0, 0.9),
                Vector3::new(0.5, -1.0, 0.3),
            ),
            TimeOfDay::Afternoon => (
                Vector3::new(0.35, 0.3, 0.25),
                Vector3::new(1.0, 0.85, 0.6),
                Vector3::new(0.4, -1.0, 0.4),
            ),
            TimeOfDay::Evening => (
                Vector3::new(0.25, 0.2, 0.2),
                Vector3::new(0.9, 0.6, 0.4),
                Vector3::new(0.3, -1.0, 0.5),
            ),
            TimeOfDay::Night => (
                Vector3::new(0.1, 0.1, 0.15),
                Vector3::new(0.3, 0.3, 0.5),
                Vector3::new(0.2, -1.0, 0.2),
            ),
        };

        // Update scene ambient light
        self.scene.write().set_ambient_light(ambient);

        // Update global directional lights
        for i in 0..self.num_global_lights {
            if let Some(ref mut light) = self.global_lights[i] {
                light.set_ambient(Vector3::new(0.0, 0.0, 0.0));
                light.set_diffuse(diffuse);
                light.set_direction(direction);
            }
        }

        // Propagate to scene
        let mut scene = self.scene.write();
        for i in 0..self.num_global_lights {
            if let Some(ref light) = self.global_lights[i] {
                scene.set_global_light(light.clone(), i);
            }
        }
    }

    /// Create a light pulse that grows, decays, and vanishes over several frames.
    ///
    /// C++ `createLightPulse(pos, color, innerRadius, outerRadius, increaseFrameTime, decayFrameTime)`
    pub fn create_light_pulse(&self, params: LightPulseParams) {
        if params.inner_radius + params.outer_radius < 2.0 * 100.0 + 1.0 {
            return; // C++: too small to make visual difference
        }

        let mut light = W3DDynamicLight::point();
        light.set_enabled(true);
        light.set_ambient(Vector3::new(params.color.0, params.color.1, params.color.2));
        light.set_diffuse(Vector3::new(params.color.0, params.color.1, params.color.2));
        light.set_position(Vector3::new(params.pos.0, params.pos.1, params.pos.2));
        light.set_range(
            params.inner_radius,
            params.inner_radius + params.outer_radius,
        );
        light.set_frame_fade(params.increase_frame_time, params.decay_frame_time);
        light.set_decay_range(true);
        light.set_decay_color(true);

        self.scene.write().add_dynamic_light(light);
    }
}

// ---------------------------------------------------------------------------
// Scene / view accessors
// ---------------------------------------------------------------------------

impl W3DDisplay {
    /// Get a clone of the Arc to the 3D scene.
    pub fn scene(&self) -> Arc<RwLock<W3DScene>> {
        Arc::clone(&self.scene)
    }

    /// Get the 2D scene.
    pub fn scene_2d(&self) -> &W3D2DScene {
        &self.scene_2d
    }

    /// Get mutable access to the 2D scene.
    pub fn scene_2d_mut(&mut self) -> &mut W3D2DScene {
        &mut self.scene_2d
    }

    /// Get the 3D interface scene.
    pub fn scene_3d_interface(&self) -> &W3DInterfaceScene {
        &self.scene_3d_interface
    }

    /// Get mutable access to the 3D interface scene.
    pub fn scene_3d_interface_mut(&mut self) -> &mut W3DInterfaceScene {
        &mut self.scene_3d_interface
    }

    /// Get the asset manager reference.
    pub fn asset_manager(&self) -> Option<&WthreeDAssetManager> {
        self.asset_manager.as_ref()
    }

    /// Get mutable asset manager reference.
    pub fn asset_manager_mut(&mut self) -> Option<&mut WthreeDAssetManager> {
        self.asset_manager.as_mut()
    }

    /// Set the primary view (called after WGPU device is created).
    pub fn set_view(&mut self, view: W3DView) {
        *self.view.write() = Some(view);
    }

    /// Check if initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get the terrain visual.
    pub fn terrain_visual(&self) -> Option<&WthreeDTerrainVisual> {
        self.terrain_visual.as_ref()
    }

    /// Get mutable terrain visual.
    pub fn terrain_visual_mut(&mut self) -> Option<&mut WthreeDTerrainVisual> {
        self.terrain_visual.as_mut()
    }

    /// Set the terrain visual (called during map load).
    pub fn set_terrain_visual(&mut self, visual: WthreeDTerrainVisual) {
        self.terrain_visual = Some(visual);
    }
}

// ---------------------------------------------------------------------------
// WGPU resource injection
// ---------------------------------------------------------------------------

impl W3DDisplay {
    /// Inject WGPU device/queue/surface created externally.
    ///
    /// C++ creates these in `WW3D::Init(ApplicationHWnd)` + `WW3D::Set_Render_Device()`.
    /// The Rust port receives pre-created WGPU resources from the application
    /// layer and stores them for render pipeline use.
    pub fn set_wgpu_resources(
        &mut self,
        device: Arc<Device>,
        queue: Arc<Queue>,
        surface: Arc<Surface>,
        config: SurfaceConfiguration,
    ) {
        let w = config.width;
        let h = config.height;
        let surface_format = config.format;

        self.render_2d = Some(Render2DPipeline::new(
            device.clone(),
            queue.clone(),
            w,
            h,
            surface_format,
        ));

        self.wgpu_device = Some(device);
        self.wgpu_queue = Some(queue);
        self.wgpu_surface = Some(surface);
        self.wgpu_surface_config = Some(config);
        self.width = self
            .wgpu_surface_config
            .as_ref()
            .map(|c| c.width)
            .unwrap_or(self.width);
        self.height = self
            .wgpu_surface_config
            .as_ref()
            .map(|c| c.height)
            .unwrap_or(self.height);
    }
}

// ---------------------------------------------------------------------------
// Render frame (C++ W3DDisplay::draw)
// ---------------------------------------------------------------------------

impl W3DDisplay {
    /// Orchestrate the full render pipeline for one frame.
    ///
    /// C++ `draw()` does:
    /// 1. updateAverageFPS()
    /// 2. Dynamic LOD selection
    /// 3. Terrain LOD calculation
    /// 4. gatherDebugStats()
    /// 5. Sync W3D time
    /// 6. updateViews()
    /// 7. Particle system update
    /// 8. Water render-target update
    /// 9. Shadow render-target update
    /// 10. WW3D::Begin_Render()
    /// 11. drawViews()
    /// 12. UI draw
    /// 13. Mouse draw
    /// 14. Letter-box
    /// 15. Debug overlay
    /// 16. FPS bar
    /// 17. WW3D::End_Render()
    pub fn render_frame(&mut self) -> Result<()> {
        let mut view_guard = self.view.write();
        let Some(view) = view_guard.as_mut() else {
            return Ok(());
        };

        // Update FPS tracking
        self.update_average_fps();

        // Build render info from the view's camera
        let mut rinfo = RenderInfo::new();
        rinfo.camera = CameraInfo {
            position: cgmath::Point3::new(
                view.camera.position.x,
                view.camera.position.y,
                view.camera.position.z,
            ),
            direction: {
                let d = view.camera.look_at - view.camera.position;
                if d.magnitude2() > 0.0 {
                    d.normalize()
                } else {
                    cgmath::Vector3::new(0.0, 0.0, -1.0)
                }
            },
            near_z: view.camera.near_plane,
            far_z: view.camera.far_plane,
            fov: view.camera.field_of_view.0,
        };

        // Add light environment
        rinfo.light_environment = Some(self.scene.read().get_default_light_env().clone());

        // Render the 3D scene (visibility, culling, logic updates)
        {
            let mut scene = self.scene.write();
            scene.render(&mut rinfo);
        }

        // GPU render pass: terrain → scene objects → particles → lines → 2D UI
        {
            let mut scene = self.scene.write();
            let terrain = self.terrain_visual.as_ref();
            let r2d = self.render_2d.as_mut();
            view.render_scene(&mut scene, terrain, r2d)?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// FPS tracking (C++ updateAverageFPS, getAverageFPS, getLastFrameDrawCalls)
// ---------------------------------------------------------------------------

impl W3DDisplay {
    /// Update the moving-average FPS measurement.
    ///
    /// C++ `updateAverageFPS()`: keeps a ring buffer of the last 30 FPS samples,
    /// ignoring frame-time spikes >= 0.5s.
    pub fn update_average_fps(&mut self) {
        let now = std::time::Instant::now();
        let elapsed_nanos = if self.last_frame_time_nanos == 0 {
            // First frame — seed with a nominal 33ms.
            33_000_000
        } else {
            now.elapsed().as_nanos() as i64
        };

        let elapsed_seconds = elapsed_nanos as f64 / 1_000_000_000.0;

        if elapsed_seconds <= MAX_FRAME_TIME_CUTOFF && elapsed_seconds > 0.0 {
            let current_fps = 1.0 / elapsed_seconds;
            let offset = self.fps_history_offset;
            self.fps_history[offset] = current_fps;
            self.fps_history_offset = (offset + 1) % FPS_HISTORY_SIZE;
            self.fps_sample_count = self
                .fps_sample_count
                .saturating_add(1)
                .min(FPS_HISTORY_SIZE);
        }

        if self.fps_sample_count > 0 {
            let mut sum = 0.0f64;
            for i in 0..self.fps_sample_count {
                let idx = if self.fps_history_offset >= i {
                    self.fps_history_offset - 1 - i
                } else {
                    self.fps_history_offset - 1 - i + FPS_HISTORY_SIZE
                };
                sum += self.fps_history[idx];
            }
            self.average_fps = (sum / self.fps_sample_count as f64) as f32;
        }
    }

    /// Get the average FPS over the last 30 frames (C++ getAverageFPS).
    pub fn get_average_fps(&self) -> f32 {
        self.average_fps
    }

    /// Get draw calls from the previous frame (C++ getLastFrameDrawCalls).
    /// Returns 0 in the Rust port until WGPU query instrumentation is wired.
    pub fn get_last_frame_draw_calls(&self) -> i32 {
        0
    }
}

// ---------------------------------------------------------------------------
// Debug overlay (C++ drawFramerateBar, gatherDebugStats, drawDebugStats, drawFPSStats)
// ---------------------------------------------------------------------------

impl W3DDisplay {
    /// Enable/disable the full debug overlay (C++ m_debugDisplayCallback).
    pub fn set_debug_display_enabled(&mut self, enabled: bool) {
        self.debug_display_enabled = enabled;
    }

    /// Check if debug display is enabled.
    pub fn is_debug_display_enabled(&self) -> bool {
        self.debug_display_enabled
    }

    /// Set benchmark timer value (C++ m_benchmarkTimer).
    pub fn set_benchmark_timer(&mut self, timer: i32) {
        self.benchmark_timer = timer;
    }

    /// Get benchmark timer value.
    pub fn get_benchmark_timer(&self) -> i32 {
        self.benchmark_timer
    }

    /// Draw the FPS bar overlay (C++ drawFramerateBar / drawFPSStats).
    ///
    /// In the full render pipeline this is called after WW3D::End_Render().
    /// The Rust port stores the data for a future overlay pass.
    pub fn draw_framerate_bar(&self) -> String {
        if self.benchmark_timer <= 0 {
            return String::new();
        }
        format!("FPS: {:.2}", self.average_fps)
    }

    /// Gather debug statistics (C++ gatherDebugStats).
    ///
    /// C++ computes FPS, frame number, polygons, vertices, video RAM,
    /// camera position, particles, objects, network stats, selected info.
    /// Returns a formatted string for display.
    pub fn gather_debug_stats(&self, frame: u32) -> String {
        if !self.debug_display_enabled && self.benchmark_timer <= 0 {
            return String::new();
        }

        let view = self.view.read();
        let cam = view.as_ref().map(|v| &v.camera);

        let mut lines = Vec::new();
        lines.push(format!(
            "{:.2} FPS, {:.2}ms draws: 0",
            self.average_fps,
            if self.average_fps > 0.0 {
                1000.0 / self.average_fps as f64
            } else {
                0.0
            }
        ));
        lines.push(format!("Frame: {}", frame));
        lines.push("Polygons: per frame 0, per second 0".to_string());
        lines.push("Vertices: 0".to_string());

        if let Some(cam) = cam {
            lines.push(format!(
                "Camera zoom: {}, pitch: {}, yaw: {}, pos: {}, {}, {}",
                cam.zoom_factor, cam.pitch, cam.yaw, cam.position.x, cam.position.y, cam.position.z,
            ));
        }

        lines.join("\n")
    }
}

// ---------------------------------------------------------------------------
// 2D drawing primitives (C++ drawLine, drawFillRect, drawOpenRect, etc.)
// ---------------------------------------------------------------------------

impl W3DDisplay {
    pub fn draw_line(
        &mut self,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        line_width: f32,
        color: u32,
    ) {
        if let Some(ref mut pipeline) = self.render_2d {
            pipeline.queue_line(
                start_x as f32,
                start_y as f32,
                end_x as f32,
                end_y as f32,
                line_width,
                color,
            );
        }
    }

    pub fn draw_line_gradient(
        &mut self,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        line_width: f32,
        color1: u32,
        _color2: u32,
    ) {
        if let Some(ref mut pipeline) = self.render_2d {
            pipeline.queue_line(
                start_x as f32,
                start_y as f32,
                end_x as f32,
                end_y as f32,
                line_width,
                color1,
            );
        }
    }

    pub fn draw_open_rect(
        &mut self,
        start_x: i32,
        start_y: i32,
        width: i32,
        height: i32,
        line_width: f32,
        color: u32,
    ) {
        if let Some(ref mut pipeline) = self.render_2d {
            pipeline.queue_open_rect(
                start_x as f32,
                start_y as f32,
                width as f32,
                height as f32,
                line_width,
                color,
            );
        }
    }

    pub fn draw_fill_rect(
        &mut self,
        start_x: i32,
        start_y: i32,
        width: i32,
        height: i32,
        color: u32,
    ) {
        if let Some(ref mut pipeline) = self.render_2d {
            pipeline.queue_rect(
                start_x as f32,
                start_y as f32,
                width as f32,
                height as f32,
                color,
            );
        }
    }

    /// Draw an image (textured quad) on the display.
    ///
    /// Corresponds to C++ `W3DDisplay::drawImage(const Image*, Int..Int, Color, DrawImageMode)`.
    /// C++ uses `Render2DClass::Add_Quad(screen_rect, uv_rect, color)` with
    /// texture set from `image->getFilename()` or `image->getRawTextureData()`.
    pub fn draw_image(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        color: u32,
        mode: DrawImageMode,
        texture_id: u64,
    ) {
        let (sx0, sy0, sx1, sy1, su0, sv0, su1, sv1) = if self.clipping_enabled {
            self.clip_image_quad(x0 as f32, y0 as f32, x1 as f32, y1 as f32, u0, v0, u1, v1)
        } else {
            (x0 as f32, y0 as f32, x1 as f32, y1 as f32, u0, v0, u1, v1)
        };

        if sx1 <= self.clip_region.lo_x as f32 || sy1 <= self.clip_region.lo_y as f32 {
            return;
        }

        if let Some(ref pipeline) = self.render_2d {
            pipeline.queue_image(
                sx0, sy0, sx1, sy1, su0, sv0, su1, sv1, color, texture_id, mode,
            );
        }
    }

    fn clip_image_quad(
        &self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
    ) -> (f32, f32, f32, f32, f32, f32, f32, f32) {
        let clip_lo_x = self.clip_region.lo_x as f32;
        let clip_lo_y = self.clip_region.lo_y as f32;
        let clip_hi_x = self.clip_region.hi_x as f32;
        let clip_hi_y = self.clip_region.hi_y as f32;

        let cx0 = x0.max(clip_lo_x);
        let cy0 = y0.max(clip_lo_y);
        let cx1 = x1.min(clip_hi_x);
        let cy1 = y1.min(clip_hi_y);

        let sw = x1 - x0;
        let sh = y1 - y0;
        if sw < 0.001 || sh < 0.001 {
            return (cx0, cy0, cx1, cy1, u0, v0, u1, v1);
        }

        let uw = u1 - u0;
        let vh = v1 - v0;
        let pu0 = (cx0 - x0) / sw;
        let pu1 = (cx1 - x0) / sw;
        let pv0 = (cy0 - y0) / sh;
        let pv1 = (cy1 - y0) / sh;

        (
            cx0,
            cy0,
            cx1,
            cy1,
            u0 + uw * pu0,
            u0 + uw * pu1,
            v0 + vh * pv0,
            v0 + vh * pv1,
        )
    }

    pub fn draw_rect_clock(
        &mut self,
        start_x: i32,
        start_y: i32,
        width: i32,
        height: i32,
        percent: i32,
        color: u32,
    ) {
        if percent < 1 || percent > 100 {
            return;
        }
        if percent == 100 {
            self.draw_fill_rect(start_x, start_y, width, height, color);
            return;
        }
        self.draw_fill_rect(start_x, start_y, width * percent / 100, height, color);
    }

    pub fn draw_remaining_rect_clock(
        &mut self,
        start_x: i32,
        start_y: i32,
        width: i32,
        height: i32,
        percent: i32,
        color: u32,
    ) {
        if percent >= 100 {
            return;
        }
        let remaining_width = width * (100 - percent) / 100;
        self.draw_fill_rect(
            start_x + width - remaining_width,
            start_y,
            remaining_width,
            height,
            color,
        );
    }

    pub fn flush_2d(&mut self, render_pass: &mut wgpu::RenderPass<'_>) {
        if let Some(ref mut pipeline) = self.render_2d {
            pipeline.flush(render_pass);
        }
    }

    pub fn render_2d_pipeline(&self) -> Option<&Render2DPipeline> {
        self.render_2d.as_ref()
    }
}

// ---------------------------------------------------------------------------
// Clipping (C++ setClipRegion, enableClipping, isClippingEnabled)
// ---------------------------------------------------------------------------

impl W3DDisplay {
    /// Set clip rectangle for 2D draw operations (C++ setClipRegion).
    pub fn set_clip_region(&mut self, region: IRegion2D) {
        self.clip_region = region;
    }

    /// Get the current clip region.
    pub fn get_clip_region(&self) -> IRegion2D {
        self.clip_region
    }

    /// Enable/disable clipping for 2D operations (C++ enableClipping).
    pub fn enable_clipping(&mut self, enabled: bool) {
        self.clipping_enabled = enabled;
    }

    /// Check if clipping is enabled (C++ isClippingEnabled).
    pub fn is_clipping_enabled(&self) -> bool {
        self.clipping_enabled
    }
}

// ---------------------------------------------------------------------------
// Letter-box (C++ toggleLetterBox, enableLetterBox, isLetterBoxed, etc.)
// ---------------------------------------------------------------------------

impl W3DDisplay {
    /// Toggle letter-box mode (C++ toggleLetterBox).
    pub fn toggle_letter_box(&mut self) {
        self.letter_box_enabled = !self.letter_box_enabled;
        self.letter_box_fade_start_time_ms = current_time_ms();
    }

    /// Force enable/disable letter-box (C++ enableLetterBox).
    pub fn enable_letter_box(&mut self, enable: bool) {
        if enable && !self.letter_box_enabled {
            self.letter_box_enabled = true;
            self.letter_box_fade_start_time_ms = current_time_ms();
        } else if !enable && self.letter_box_enabled {
            self.letter_box_enabled = false;
            self.letter_box_fade_start_time_ms = current_time_ms();
        }
    }

    /// Check if letter-boxed (C++ isLetterBoxed).
    pub fn is_letter_boxed(&self) -> bool {
        self.letter_box_enabled
    }

    /// Check if letter-box is currently fading (C++ isLetterBoxFading).
    pub fn is_letter_box_fading(&self) -> bool {
        (self.letter_box_enabled && self.letter_box_fade_level != 1.0)
            || (!self.letter_box_enabled && self.letter_box_fade_level != 0.0)
    }

    /// Update and render the letter-box bars (C++ renderLetterBox).
    ///
    /// Returns (top_height, bottom_height, alpha) for the 2D render pass,
    /// or None if no letter-box is visible.
    pub fn render_letter_box(&mut self) -> Option<(f32, f32, f32)> {
        let now = current_time_ms();

        if self.letter_box_enabled {
            // Fading in
            self.letter_box_fade_level = ((now - self.letter_box_fade_start_time_ms) as f32
                / LETTER_BOX_FADE_TIME_MS)
                .min(1.0);
        } else if self.letter_box_fade_level > 0.0 {
            // Fading out
            self.letter_box_fade_level =
                1.0 - ((now - self.letter_box_fade_start_time_ms) as f32 / LETTER_BOX_FADE_TIME_MS);
            if self.letter_box_fade_level < 0.0 {
                self.letter_box_fade_level = 0.0;
            }
        }

        if self.letter_box_fade_level <= 0.0 {
            return None;
        }

        // C++: drawFillRect for top and bottom bars
        // height = (screenH - 9/16 * screenW) * 0.5
        let w = self.width as f32;
        let h = self.height as f32;
        let bar_height = (h - (9.0 / 16.0) * w) * 0.5 * self.letter_box_fade_level;

        Some((bar_height, bar_height, self.letter_box_fade_level))
    }
}

// ---------------------------------------------------------------------------
// Screenshot / movie capture (C++ takeScreenShot, toggleMovieCapture)
// ---------------------------------------------------------------------------

impl W3DDisplay {
    /// Capture the current frame to a screenshot file.
    ///
    /// C++ saves via DX8 surface capture to disk. In WGPU, this requires
    /// reading back the swapchain texture after the last submit.
    /// Placeholder — actual implementation needs WGPU texture readback.
    pub fn take_screenshot(&self) -> Result<String> {
        // PARITY_NOTE: C++ captures the backbuffer and saves to a timestamped BMP.
        // WGPU readback requires mapping a buffer after the frame.
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let filename = format!("screenshot_{timestamp}.png");
        log::info!("Screenshot requested: {}", filename);
        Ok(filename)
    }

    /// Toggle AVI/movie frame capture mode (C++ toggleMovieCapture).
    pub fn toggle_movie_capture(&mut self) {
        self.movie_capture_enabled = !self.movie_capture_enabled;
    }

    /// Check if movie capture is active.
    pub fn is_movie_capture_enabled(&self) -> bool {
        self.movie_capture_enabled
    }
}

// ---------------------------------------------------------------------------
// Time of day accessor
// ---------------------------------------------------------------------------

impl W3DDisplay {
    /// Get current time of day.
    pub fn get_time_of_day(&self) -> TimeOfDay {
        self.time_of_day
    }
}

// ---------------------------------------------------------------------------
// Default trait implementations
// ---------------------------------------------------------------------------

impl Default for W3DDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for W3DDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("W3DDisplay")
            .field("initialized", &self.initialized)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("windowed", &self.windowed)
            .field("time_of_day", &self.time_of_day)
            .field("average_fps", &self.average_fps)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Get current time in milliseconds (C++ timeGetTime / GetTickCount).
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_creation() {
        let display = W3DDisplay::new();
        assert!(!display.is_initialized());
        assert_eq!(display.get_width(), MIN_DISPLAY_RESOLUTION_X);
        assert_eq!(display.get_height(), MIN_DISPLAY_RESOLUTION_Y);
        assert_eq!(display.get_bit_depth(), DEFAULT_BIT_DEPTH);
    }

    #[test]
    fn test_display_init() {
        let mut display = W3DDisplay::new();
        display.init().unwrap();
        assert!(display.is_initialized());
        assert!(display.asset_manager().is_some());
    }

    #[test]
    fn test_set_resolution() {
        let mut display = W3DDisplay::new();
        assert!(display.set_display_mode(1024, 768, 32, true));
        assert_eq!(display.get_width(), 1024);
        assert_eq!(display.get_height(), 768);
        assert!(display.is_windowed());

        // Reject too-small resolution
        assert!(!display.set_display_mode(400, 300, 32, false));
    }

    #[test]
    fn test_set_gamma_windowed_rejected() {
        let mut display = W3DDisplay::new();
        display.set_windowed(true);
        display.set_gamma(1.5, 0.0, 1.0, false);
        // Gamma should remain at default since windowed
        assert!((display.get_gamma() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_set_gamma_fullscreen() {
        let mut display = W3DDisplay::new();
        display.set_windowed(false);
        display.set_gamma(1.5, 0.0, 1.0, false);
        assert!((display.get_gamma() - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_set_time_of_day() {
        let mut display = W3DDisplay::new();
        display.init().unwrap();

        display.set_time_of_day(TimeOfDay::Night);
        assert_eq!(display.get_time_of_day(), TimeOfDay::Night);
    }

    #[test]
    fn test_clipping() {
        let mut display = W3DDisplay::new();
        assert!(!display.is_clipping_enabled());
        display.enable_clipping(true);
        assert!(display.is_clipping_enabled());
        display.set_clip_region(IRegion2D::new(10, 10, 100, 100));
        let region = display.get_clip_region();
        assert_eq!(region.lo_x, 10);
    }

    #[test]
    fn test_letter_box_toggle() {
        let mut display = W3DDisplay::new();
        assert!(!display.is_letter_boxed());
        display.toggle_letter_box();
        assert!(display.is_letter_boxed());
        display.toggle_letter_box();
        assert!(!display.is_letter_boxed());
    }

    #[test]
    fn test_display_mode_count() {
        let display = W3DDisplay::new();
        assert_eq!(display.get_display_mode_count(), 3);
    }

    #[test]
    fn test_display_mode_description() {
        let display = W3DDisplay::new();
        assert_eq!(
            display.get_display_mode_description(0),
            Some((800, 600, 32))
        );
        assert_eq!(
            display.get_display_mode_description(2),
            Some((1280, 1024, 32))
        );
        assert_eq!(display.get_display_mode_description(99), None);
    }

    #[test]
    fn test_movie_capture_toggle() {
        let mut display = W3DDisplay::new();
        assert!(!display.is_movie_capture_enabled());
        display.toggle_movie_capture();
        assert!(display.is_movie_capture_enabled());
        display.toggle_movie_capture();
        assert!(!display.is_movie_capture_enabled());
    }

    #[test]
    fn test_global_singleton() {
        let a = W3DDisplay::global();
        let b = W3DDisplay::global();
        // Both should point to the same instance
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn test_default_trait() {
        let display = W3DDisplay::default();
        assert!(!display.is_initialized());
    }
}
