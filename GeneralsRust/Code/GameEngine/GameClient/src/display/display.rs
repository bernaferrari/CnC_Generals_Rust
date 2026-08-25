/*
**  Command & Conquer Generals Zero Hour™
*/

//! Display adaptor that renders through the shared `PlatformContext`.

use crate::display::DisplayInterface;
use crate::display::display_fx;
use crate::display::shadow_pass;
use crate::display::view::{
    LETTER_BOX_FADE_TIME_MS, ViewTrait, set_display_letter_boxed, vertical_fov_from_horizontal,
    with_tactical_view, with_tactical_view_ref,
};
use crate::drawable::drawable_manager::DrawableManager;
use crate::effects::particle_manager::get_particle_system_manager;
use crate::effects::particle_renderer::{
    ParticleRenderer as GpuParticleRenderer, ParticleUniforms, register_particle_renderer,
};
use crate::effects::weather_complete::get_weather_system;
use crate::fx_list::get_decal_manager;
use crate::game_text::GameText;
use crate::gui::display_string::{DisplayStringHandle, get_display_string_manager};
use crate::gui::font::{FontDesc, get_font_library};
use crate::gui::{ui_renderer::UIRect, with_ui_renderer, with_window_manager};
use crate::input::with_mouse;
use crate::platform::GraphicsContext;
use crate::radius_decal::get_projected_shadow_manager;
use crate::system::SubsystemInterface;
use crate::system::debug_display::DebugDisplay;
use crate::terrain::TerrainVisual;
use crate::terrain::terrain_visual::THE_TERRAIN_VISUAL;
use crate::video_buffer::{SoftwareVideoBuffer, VideoBuffer, VideoBufferType};
use crate::video_player::{VideoPlayerInterface, get_video_player};
use crate::video_stream::VideoStreamInterface;
#[cfg(feature = "w3d_support")]
use crate::w3d::W3DParticleSystemBridge;
use game_engine::common::ini::ini_game_data::{
    GlobalData, MAX_GLOBAL_LIGHTS, TIME_OF_DAY_COUNT, TimeOfDay, get_global_data,
};
use gamelogic::helpers::{TheGameLogic, TheScriptEngine};
use log::{error, warn};
use nalgebra::{Matrix4, Point3, Vector3};
use std::any::Any;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wgpu::SurfaceTexture;

pub type DebugDisplayCallback = fn(&mut DebugDisplay, Option<&mut dyn Any>);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayDirectionalLightState {
    pub ambient: [f32; 3],
    pub diffuse: [f32; 3],
    pub specular: [f32; 3],
    pub position: [f32; 3],
}

impl Default for DisplayDirectionalLightState {
    fn default() -> Self {
        Self {
            ambient: [0.0, 0.0, 0.0],
            diffuse: [0.0, 0.0, 0.0],
            specular: [0.0, 0.0, 0.0],
            position: [0.0, 0.0, -1.0],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayLightingState {
    pub time_of_day: TimeOfDay,
    pub scene_ambient: [f32; 3],
    pub lights: [DisplayDirectionalLightState; MAX_GLOBAL_LIGHTS],
    pub active_light_count: usize,
    pub terrain_time_of_day: TimeOfDay,
    pub tactical_view_redraw_forced: bool,
}

impl Default for DisplayLightingState {
    fn default() -> Self {
        Self {
            time_of_day: TimeOfDay::default(),
            scene_ambient: [0.0, 0.0, 0.0],
            lights: [DisplayDirectionalLightState::default(); MAX_GLOBAL_LIGHTS],
            active_light_count: 0,
            terrain_time_of_day: TimeOfDay::default(),
            tactical_view_redraw_forced: false,
        }
    }
}

impl DisplayLightingState {
    pub fn from_current_global_data() -> Self {
        let Some(global_data) = get_global_data() else {
            return Self::default();
        };
        let global_data = global_data.read();
        Self::from_global_data(&global_data, global_data.time_of_day, false)
    }

    pub fn from_global_data(
        global_data: &GlobalData,
        time_of_day: TimeOfDay,
        tactical_view_redraw_forced: bool,
    ) -> Self {
        let time_of_day = match time_of_day {
            TimeOfDay::Invalid => global_data.time_of_day,
            other => other,
        };
        let time_index = time_of_day_index(time_of_day);
        let mut lights = [DisplayDirectionalLightState::default(); MAX_GLOBAL_LIGHTS];
        let active_light_count = global_data
            .num_global_lights
            .clamp(0, MAX_GLOBAL_LIGHTS as i32) as usize;

        for (index, light) in lights.iter_mut().enumerate().take(active_light_count) {
            let source = &global_data.terrain_objects_lighting[time_index][index];
            *light = DisplayDirectionalLightState {
                ambient: [0.0, 0.0, 0.0],
                diffuse: [source.diffuse.r, source.diffuse.g, source.diffuse.b],
                specular: [0.0, 0.0, 0.0],
                position: [source.light_pos.x, source.light_pos.y, source.light_pos.z],
            };
        }

        let scene_source = &global_data.terrain_objects_lighting[time_index][0];
        Self {
            time_of_day,
            scene_ambient: [
                scene_source.ambient.r,
                scene_source.ambient.g,
                scene_source.ambient.b,
            ],
            lights,
            active_light_count,
            terrain_time_of_day: time_of_day,
            tactical_view_redraw_forced,
        }
    }
}

fn time_of_day_index(time_of_day: TimeOfDay) -> usize {
    match time_of_day {
        TimeOfDay::Invalid => TimeOfDay::default() as usize,
        other => (other as usize).min(TIME_OF_DAY_COUNT - 1),
    }
}

pub struct Display {
    graphics: GraphicsContext,
    particle_renderer: Option<Arc<Mutex<GpuParticleRenderer>>>,
    #[cfg(feature = "w3d_support")]
    particle_bridge: Mutex<W3DParticleSystemBridge>,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    start_time: Instant,
    width: u32,
    height: u32,
    bit_depth: u32,
    windowed: bool,
    view_list: Vec<Box<dyn ViewTrait>>,
    video_buffer: Option<Box<dyn VideoBuffer + Send>>,
    video_stream: Option<Box<dyn VideoStreamInterface>>,
    currently_playing_movie: String,
    movie_capture_enabled: bool,
    movie_hold_time_ms: i32,
    copyright_hold_time_ms: i32,
    movie_start_time: Option<Instant>,
    copyright_start_time: Option<Instant>,
    copyright_display_string: Option<DisplayStringHandle>,
    debug_display: Option<DebugDisplay>,
    debug_display_callback: Option<DebugDisplayCallback>,
    debug_display_user_data: Option<Box<dyn Any + Send + Sync>>,
    border_shroud_level: u8,
    letterbox_fade_level: f32,
    letterbox_enabled: bool,
    letterbox_fade_start_time: Option<Instant>,
    drawable_manager: Arc<Mutex<DrawableManager>>,
    lighting_state: DisplayLightingState,
    last_movie_frame: Mutex<Option<(u32, u32, Vec<u8>)>>,
    pending_screenshot: Mutex<Option<std::path::PathBuf>>,
}

impl Display {
    pub fn new(graphics: GraphicsContext) -> Self {
        let (surface_format, width, height) = {
            let config = graphics.config();
            (config.format, config.width, config.height)
        };
        let depth_texture = Self::create_depth_texture(&graphics);
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let particle_renderer = match GpuParticleRenderer::new_with_depth_format(
            graphics.device_arc(),
            graphics.queue_arc(),
            surface_format,
            wgpu::TextureFormat::Depth32Float,
        ) {
            Ok(renderer) => {
                let renderer = Arc::new(Mutex::new(renderer));
                register_particle_renderer(Arc::clone(&renderer));
                Some(renderer)
            }
            Err(err) => {
                error!("Failed to initialize particle renderer: {}", err);
                None
            }
        };

        // Initialize the drawable draw pipeline (for units/buildings geometry)
        if let Ok(drawable_pipeline) =
            crate::drawable::drawable_draw_pipeline::DrawableDrawPipeline::new(
                graphics.device_arc(),
                graphics.queue_arc(),
                surface_format,
            )
        {
            let drawable_pipeline = Arc::new(Mutex::new(drawable_pipeline));
            crate::drawable::drawable_draw_pipeline::register_drawable_pipeline(drawable_pipeline);
        } else {
            error!("Failed to initialize drawable draw pipeline");
        }
        Self {
            graphics,
            particle_renderer,
            #[cfg(feature = "w3d_support")]
            particle_bridge: Mutex::new(W3DParticleSystemBridge::new()),
            depth_texture,
            depth_view,
            start_time: Instant::now(),
            width,
            height,
            bit_depth: 32,
            windowed: true,
            view_list: Vec::new(),
            video_buffer: None,
            video_stream: None,
            currently_playing_movie: String::new(),
            movie_capture_enabled: false,
            movie_hold_time_ms: -1,
            copyright_hold_time_ms: -1,
            movie_start_time: None,
            copyright_start_time: None,
            copyright_display_string: None,
            debug_display: None,
            debug_display_callback: None,
            debug_display_user_data: None,
            border_shroud_level: 0,
            letterbox_fade_level: 0.0,
            letterbox_enabled: false,
            letterbox_fade_start_time: None,
            drawable_manager: Arc::new(Mutex::new(DrawableManager::new())),
            lighting_state: DisplayLightingState::from_current_global_data(),
            last_movie_frame: Mutex::new(None),
            pending_screenshot: Mutex::new(None),
        }
    }

    pub fn lighting_state(&self) -> &DisplayLightingState {
        &self.lighting_state
    }

    pub fn set_time_of_day(&mut self, time_of_day: TimeOfDay) -> bool {
        let Some(global_data) = get_global_data() else {
            return false;
        };
        let global_data = global_data.read();
        let effective_time_of_day = match time_of_day {
            TimeOfDay::Invalid => global_data.time_of_day,
            other => other,
        };
        if matches!(effective_time_of_day, TimeOfDay::Invalid) {
            return false;
        }

        self.lighting_state =
            DisplayLightingState::from_global_data(&global_data, effective_time_of_day, true);
        let time_index = time_of_day_index(effective_time_of_day);
        let terrain_lighting = &global_data.terrain_lighting[time_index][0];
        if let Ok(mut terrain_guard) = THE_TERRAIN_VISUAL.lock() {
            if let Some(terrain) = terrain_guard.as_mut() {
                terrain.set_lighting(
                    Some([
                        terrain_lighting.light_pos.x,
                        terrain_lighting.light_pos.y,
                        terrain_lighting.light_pos.z,
                    ]),
                    Some([
                        terrain_lighting.diffuse.r,
                        terrain_lighting.diffuse.g,
                        terrain_lighting.diffuse.b,
                    ]),
                    Some([
                        terrain_lighting.ambient.r,
                        terrain_lighting.ambient.g,
                        terrain_lighting.ambient.b,
                    ]),
                    None,
                    None,
                );
            }
        }
        with_tactical_view(|view| view.force_redraw());
        true
    }

    pub fn set_border_shroud_level(&mut self, level: u8) {
        self.border_shroud_level = level;
    }

    pub fn border_shroud_level(&self) -> u8 {
        self.border_shroud_level
    }

    pub fn begin_frame(&self) -> Result<(SurfaceTexture, wgpu::TextureView), wgpu::SurfaceError> {
        let frame = self.graphics.surface().get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        Ok((frame, view))
    }

    fn create_depth_texture(graphics: &GraphicsContext) -> wgpu::Texture {
        let config = graphics.config();
        graphics.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("Display Depth Texture"),
            size: wgpu::Extent3d {
                width: config.width.max(1),
                height: config.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    }

    fn nalgebra_to_game_matrix(nm: &nalgebra::Matrix4<f32>) -> crate::drawable::Matrix4 {
        let mut gm = crate::drawable::Matrix4::identity();
        for r in 0..4 {
            for c in 0..4 {
                gm.elements[r][c] = nm[(r, c)];
            }
        }
        gm
    }

    pub fn attach_view(&mut self, view: Box<dyn ViewTrait>) {
        self.view_list.push(view);
    }

    pub fn delete_views(&mut self) {
        self.view_list.clear();
    }

    pub fn draw_views(&self) {
        for view in &self.view_list {
            let _ = view.draw_view();
        }
    }

    pub fn update_views(&mut self) {
        for view in &mut self.view_list {
            let _ = view.update_view();
        }
    }

    pub fn reset_views(&mut self) {
        for view in &mut self.view_list {
            view.reset_view();
        }
    }

    pub fn drawable_manager(&self) -> Arc<Mutex<DrawableManager>> {
        Arc::clone(&self.drawable_manager)
    }

    pub fn set_width(&mut self, width: u32) {
        self.width = width;
        self.update_mouse_limits();
    }

    pub fn set_height(&mut self, height: u32) {
        self.height = height;
        self.update_mouse_limits();
    }

    fn update_mouse_limits(&self) {
        with_mouse(|mouse| mouse.set_mouse_limits(self.width, self.height));
    }

    pub fn set_bit_depth(&mut self, bit_depth: u32) {
        self.bit_depth = bit_depth;
    }

    pub fn set_windowed(&mut self, windowed: bool) {
        self.windowed = windowed;
    }

    pub fn get_width(&self) -> u32 {
        self.width
    }

    pub fn get_height(&self) -> u32 {
        self.height
    }

    pub fn get_bit_depth(&self) -> u32 {
        self.bit_depth
    }

    pub fn is_windowed(&self) -> bool {
        self.windowed
    }

    pub fn set_display_mode(
        &mut self,
        xres: u32,
        yres: u32,
        bit_depth: u32,
        windowed: bool,
    ) -> bool {
        // C++ W3DDisplay::setDisplayMode — Set_Device_Resolution then rescale view.
        // Failure restores the previous device mode and returns FALSE.
        if xres == 0 || yres == 0 {
            return false;
        }
        let old_width = self.width;
        let old_height = self.height;
        let old_bit_depth = self.bit_depth;
        let old_windowed = self.windowed;
        let old_display_width = old_width.max(1) as f32;
        let old_display_height = old_height.max(1) as f32;
        let (old_view_width, old_view_height, old_view_origin_x, old_view_origin_y) =
            with_tactical_view(|view| {
                (
                    view.width() as f32,
                    view.height() as f32,
                    view.origin().0 as f32,
                    view.origin().1 as f32,
                )
            });

        self.graphics.resize(xres, yres);
        self.rebuild_depth_resources();
        let _ = crate::gui::ui_globals::with_ui_renderer_mut(|renderer| {
            renderer.set_screen_size(xres, yres);
        });
        // Live device is Main/ww3d. Queue Set_Device_Resolution for host present.
        display_fx::queue_device_mode(display_fx::PendingDeviceMode {
            xres,
            yres,
            bit_depth,
            windowed,
        });

        self.set_width(xres);
        self.set_height(yres);
        self.set_bit_depth(bit_depth);
        self.set_windowed(windowed);

        with_tactical_view(|view| {
            view.set_width(((old_view_width / old_display_width) * xres as f32) as i32);
            view.set_height(((old_view_height / old_display_height) * yres as f32) as i32);
            view.set_origin(
                ((old_view_origin_x / old_display_width) * xres as f32) as i32,
                ((old_view_origin_y / old_display_height) * yres as f32) as i32,
            );
        });
        let _ = (old_bit_depth, old_windowed);
        true
    }

    fn rebuild_depth_resources(&mut self) {
        self.depth_texture = Self::create_depth_texture(&self.graphics);
        self.depth_view = self
            .depth_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
    }

    /// C++ `W3DDisplay::getDisplayModeCount`.
    pub fn get_display_mode_count(&self) -> i32 {
        display_fx::display_mode_count()
    }

    /// C++ `W3DDisplay::getDisplayModeDescription`.
    pub fn get_display_mode_description(&self, mode_index: i32) -> Option<(u32, u32, u32)> {
        display_fx::display_mode_description(mode_index)
    }

    pub fn create_video_buffer(&self) -> Box<dyn VideoBuffer + Send> {
        Box::new(SoftwareVideoBuffer::new(VideoBufferType::X8R8G8B8))
    }

    pub fn play_logo_movie(
        &mut self,
        movie_name: String,
        min_movie_length: i32,
        min_copyright_length: i32,
    ) {
        self.stop_movie();

        let stream = self.open_video_stream(movie_name.clone());
        let Some(stream) = stream else {
            warn!("logo movie skipped (no video provider): {}", movie_name);
            return;
        };

        self.currently_playing_movie = movie_name;
        self.movie_hold_time_ms = min_movie_length;
        self.copyright_hold_time_ms = min_copyright_length;
        self.movie_start_time = Some(Instant::now());
        self.copyright_start_time = None;

        let mut buffer = self.create_video_buffer();
        if !buffer.allocate(stream.width() as u32, stream.height() as u32) {
            self.stop_movie();
            return;
        }
        self.video_buffer = Some(buffer);
        self.video_stream = Some(stream);
    }

    pub fn play_movie(&mut self, movie_name: String) -> bool {
        self.stop_movie();

        let stream = self.open_video_stream(movie_name.clone());
        let Some(stream) = stream else {
            warn!("movie playback skipped (no video provider): {}", movie_name);
            return false;
        };

        self.currently_playing_movie = movie_name;

        let mut buffer = self.create_video_buffer();
        if !buffer.allocate(stream.width() as u32, stream.height() as u32) {
            self.stop_movie();
            return false;
        }

        self.video_buffer = Some(buffer);
        self.video_stream = Some(stream);
        true
    }

    pub fn stop_movie(&mut self) {
        // C++ Display::stopMovie intended TheScriptEngine->notifyOfCompletedVideo
        // before clearing m_currentlyPlayingMovie (commented for MP sync).
        // Single-player HAS_FINISHED_VIDEO waits leftover m_completedVideo.
        let completed = if self.currently_playing_movie.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.currently_playing_movie))
        };
        self.video_buffer = None;
        display_fx::clear_movie_frame();
        display_fx::set_copyright_overlay(None);

        if let Some(stream) = self.video_stream.take() {
            stream.close();
        }

        if let Some(display_string) = self.copyright_display_string.take() {
            let mut manager = get_display_string_manager();
            manager.free_display_string(display_string);
        }

        self.movie_hold_time_ms = -1;
        self.copyright_hold_time_ms = -1;
        self.movie_start_time = None;
        self.copyright_start_time = None;

        TheGameLogic::set_intro_movie_playing(false);
        if let Some(name) = completed {
            TheScriptEngine::notify_of_completed_video(&name);
        }
    }

    pub fn is_movie_playing(&self) -> bool {
        self.video_stream.is_some() && self.video_buffer.is_some()
    }

    pub fn toggle_movie_capture(&mut self) {
        self.movie_capture_enabled = !self.movie_capture_enabled;
        display_fx::set_movie_capture_enabled(self.movie_capture_enabled);
        if self.movie_capture_enabled {
            display_fx::reset_movie_capture_counter();
        }
    }

    pub fn is_movie_capture_enabled(&self) -> bool {
        self.movie_capture_enabled || display_fx::is_movie_capture_enabled()
    }

    /// C++ `W3DDisplay::setGamma` stores the ramp; wgpu remaps our frame only
    /// (unlike DX8 `Set_Gamma`, it cannot tint the desktop, so windowed is safe).
    pub fn set_gamma(&mut self, gamma: f32, bright: f32, contrast: f32, _calibrate: bool) {
        display_fx::set_gamma_state(gamma, bright, contrast);
    }

    /// C++ `W3DDisplay::takeScreenShot` — queue a backbuffer dump, not the movie buffer.
    pub fn take_screenshot(&mut self) -> String {
        let path = display_fx::next_screenshot_path();
        let leaf = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("sshot.bmp")
            .to_string();
        if let Ok(mut pending) = self.pending_screenshot.lock() {
            *pending = Some(path.clone());
        }
        display_fx::queue_screenshot(path);
        crate::helpers::TheInGameUI::message(&format!(
            "{} {}",
            crate::game_text::GameText::fetch("GUI:ScreenCapture"),
            leaf
        ));
        leaf
    }

    pub fn set_clip_region(&mut self, lo_x: f32, lo_y: f32, hi_x: f32, hi_y: f32) {
        display_fx::set_clip_region(lo_x, lo_y, hi_x, hi_y);
    }

    pub fn enable_clipping(&mut self, enabled: bool) {
        display_fx::enable_clipping(enabled);
    }

    /// C++ `W3DDisplay::drawImage` — ALPHA / GRAYSCALE / ADDITIVE / SOLID + rotate-90.
    pub fn draw_image(
        &self,
        image: &crate::display::image::Image,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        color: u32,
        mode: display_fx::DrawImageMode,
    ) {
        let uv = image.get_uv();
        let rotated = image
            .get_status()
            .contains(crate::display::image::ImageStatus::ROTATED_90_CLOCKWISE);
        let Some(gpu) = image.get_gpu_texture() else {
            return;
        };
        let texture = std::sync::Arc::new(gpu.view().clone());
        let _ = display_fx::queue_draw_image_mesh(
            texture,
            start_x as f32,
            start_y as f32,
            end_x as f32,
            end_y as f32,
            uv.min.x,
            uv.min.y,
            uv.max.x,
            uv.max.y,
            display_fx::color_u32_to_rgba(color),
            mode,
            rotated,
        );
    }

    pub fn enable_letter_box(&mut self, enabled: bool) {
        if self.letterbox_enabled == enabled {
            return;
        }
        self.letterbox_enabled = enabled;
        self.letterbox_fade_start_time = Some(Instant::now());
        display_fx::set_letterbox_enabled(enabled);
        set_display_letter_boxed(enabled);
        with_tactical_view(|view| view.set_zoom_limited(!enabled));
    }

    pub fn toggle_letter_box(&mut self) {
        self.enable_letter_box(!self.letterbox_enabled);
    }

    pub fn is_letter_box_enabled(&self) -> bool {
        self.letterbox_enabled
    }

    pub fn is_letter_boxed(&self) -> bool {
        self.letterbox_enabled
    }

    pub fn is_letter_box_fading(&self) -> bool {
        let fade = self.letterbox_fade_now();
        (self.letterbox_enabled && fade < 1.0) || (!self.letterbox_enabled && fade > 0.0)
    }

    fn letterbox_fade_now(&self) -> f32 {
        let Some(start) = self.letterbox_fade_start_time else {
            return if self.letterbox_enabled { 1.0 } else { 0.0 };
        };
        let t = (start.elapsed().as_millis() as f32 / LETTER_BOX_FADE_TIME_MS).clamp(0.0, 1.0);
        if self.letterbox_enabled { t } else { 1.0 - t }
    }

    /// C++ `W3DDisplay::renderLetterBox` — constant 16:9 height, alpha fade.
    fn queue_letterbox_and_filters(&self, renderer: &mut crate::gui::ui_renderer::UIRenderer) {
        let fade = self.letterbox_fade_now();
        let width = self.width.max(1) as f32;
        let height = self.height.max(1) as f32;
        let plan = display_fx::letterbox_plan(width, height, fade, self.letterbox_enabled);
        if plan.draw_top {
            renderer.draw_rect(
                UIRect::new(0.0, 0.0, width, plan.bar_height),
                plan.color,
                10_000.0,
            );
        }
        if plan.draw_bottom {
            renderer.draw_rect(
                UIRect::new(0.0, height - plan.bar_height, width, plan.bar_height),
                plan.color,
                10_000.0,
            );
        }
        // Viewport filters composite via shader_filter RTT (W3DShaderManager
        // startRenderToTexture / filterPostRender), not a UI color quad.
        // Mouse is drawn by TheMouse / host cursor — never a leftover white quad.
    }

    pub fn set_debug_display_callback(
        &mut self,
        callback: Option<DebugDisplayCallback>,
        user_data: Option<Box<dyn Any + Send + Sync>>,
    ) {
        self.debug_display_callback = callback;
        self.debug_display_user_data = user_data;
    }

    pub fn get_debug_display_callback(&self) -> Option<DebugDisplayCallback> {
        self.debug_display_callback
    }

    fn open_video_stream(&self, movie_title: String) -> Option<Box<dyn VideoStreamInterface>> {
        let player = get_video_player()?;
        let mut guard = player.lock().ok()?;
        let player = guard.as_mut()?;
        player.open(movie_title)
    }

    fn ensure_copyright_display_string(&mut self) {
        let mut manager = get_display_string_manager();
        let display_string = manager.new_display_string();
        let (font_name, font_size, font_bold) = {
            let lang = crate::global_language::get_global_language_data()
                .read()
                .ok();
            if let Some(lang) = lang.as_ref() {
                if !lang.copyright_font.name.is_empty() {
                    (
                        lang.copyright_font.name.clone(),
                        lang.adjust_font_size(lang.copyright_font.size),
                        lang.copyright_font.bold,
                    )
                } else {
                    ("Courier".to_string(), lang.adjust_font_size(12), true)
                }
            } else {
                ("Courier".to_string(), 12, true)
            }
        };
        let (text_w, text_h) = {
            let mut display_string_mut = display_string.borrow_mut();
            display_string_mut.set_text(GameText::fetch("GUI:EACopyright"));
            let font_desc = FontDesc::new(&font_name, font_size, font_bold);
            let mut font_library = get_font_library();
            if let Ok(font) = font_library.get_font(&font_desc) {
                display_string_mut.set_font(font);
            }
            display_string_mut.get_size()
        };
        display_fx::set_copyright_overlay(Some(display_fx::CopyrightOverlay {
            text: GameText::fetch("GUI:EACopyright"),
            width: text_w as f32,
            height: text_h.max(font_size) as f32,
            font_size: font_size.max(1) as f32,
        }));
        self.copyright_display_string = Some(display_string);
        self.copyright_start_time = Some(Instant::now());
    }

    fn update_movie_playback(&mut self) {
        let should_stop = {
            let (stream, buffer) = match (self.video_stream.as_mut(), self.video_buffer.as_mut()) {
                (Some(stream), Some(buffer)) => (stream, buffer),
                _ => return,
            };

            if !stream.is_frame_ready() {
                return;
            }

            stream.frame_decompress();
            stream.frame_render(buffer.as_mut());
            if let Some((w, h, rgba)) = display_fx::video_buffer_rgba_mut(buffer.as_mut()) {
                display_fx::store_movie_frame(w, h, rgba.clone());
                if let Ok(mut frame) = self.last_movie_frame.lock() {
                    *frame = Some((w, h, rgba));
                }
            }

            if stream.frame_index() != stream.frame_count() - 1 {
                stream.frame_next();
                return;
            }

            if self.copyright_hold_time_ms >= 0 || self.movie_hold_time_ms >= 0 {
                if self.copyright_start_time.is_none() && self.copyright_hold_time_ms >= 0 {
                    self.ensure_copyright_display_string();
                }

                let now = Instant::now();
                let movie_elapsed = self
                    .movie_start_time
                    .map(|start| now.duration_since(start))
                    .unwrap_or_else(|| Duration::from_millis(0));
                let copyright_elapsed = self
                    .copyright_start_time
                    .map(|start| now.duration_since(start))
                    .unwrap_or_else(|| Duration::from_millis(0));

                if (self.movie_hold_time_ms >= 0
                    && movie_elapsed
                        >= Duration::from_millis(self.movie_hold_time_ms.max(0) as u64))
                    && (self.copyright_hold_time_ms >= 0
                        && copyright_elapsed
                            >= Duration::from_millis(self.copyright_hold_time_ms.max(0) as u64))
                {
                    self.movie_hold_time_ms = -1;
                    self.copyright_hold_time_ms = -1;
                    self.movie_start_time = None;
                    self.copyright_start_time = None;
                }
                false
            } else {
                true
            }
        };

        if should_stop {
            self.stop_movie();
        }
    }

    fn build_particle_uniforms(&self) -> ParticleUniforms {
        with_tactical_view_ref(|view| {
            let camera_pos = view.get_3d_camera_position();
            let target = view.position();
            let camera = Point3::new(camera_pos.x, camera_pos.y, camera_pos.z);
            let target = Point3::new(target.x, target.y, target.z);
            let up = Vector3::new(0.0, 0.0, 1.0);

            let view_matrix = Matrix4::look_at_rh(&camera, &target, &up);
            let aspect = (view.width() as f32 / view.height().max(1) as f32).max(0.01);
            let projection_matrix = Matrix4::new_perspective(
                aspect,
                vertical_fov_from_horizontal(view.field_of_view(), aspect),
                1.0,
                20000.0,
            );

            ParticleUniforms {
                view_matrix: view_matrix.into(),
                projection_matrix: projection_matrix.into(),
                camera_position: [camera.x, camera.y, camera.z],
                time: self.start_time.elapsed().as_secs_f32(),
                screen_size: [
                    self.graphics.config().width as f32,
                    self.graphics.config().height as f32,
                ],
                particle_count: 0,
                _padding: 0,
            }
        })
    }
}

impl SubsystemInterface for Display {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(global_data) = get_global_data() {
            let (time_of_day, gamma) = {
                let gd = global_data.read();
                (gd.time_of_day, gd.display_gamma)
            };
            self.set_time_of_day(time_of_day);
            if (gamma - 1.0).abs() > 0.0001 {
                self.set_gamma(gamma, 0.0, 1.0, false);
            }
        }
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.letterbox_fade_level = 0.0;
        self.letterbox_enabled = false;
        self.letterbox_fade_start_time = None;
        self.stop_movie();
        self.reset_views();
        #[cfg(feature = "w3d_support")]
        {
            self.particle_bridge = Mutex::new(W3DParticleSystemBridge::new());
        }
        if let Some(global_data) = get_global_data() {
            let time_of_day = global_data.read().time_of_day;
            self.set_time_of_day(time_of_day);
        }
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.update_views();
        self.update_movie_playback();
        #[cfg(feature = "w3d_support")]
        if let Ok(mut bridge) = self.particle_bridge.lock() {
            bridge.queue_particle_render();
        }
        Ok(())
    }
}

impl DisplayInterface for Display {
    fn draw(&self) -> Result<(), Box<dyn std::error::Error>> {
        crate::display::client_draw_schedule::run_dual_world_cpu_phases();

        let (frame, view) = self
            .begin_frame()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        self.draw_views();

        let mut encoder =
            self.graphics
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Display Render Encoder"),
                });

        let filter_composite = with_tactical_view_ref(|v| v.filter_composite());
        let rtt_view = crate::display::shader_filter::start_render_to_texture(
            self.graphics.device(),
            self.graphics.queue(),
            self.graphics.config().format,
            self.width.max(1),
            self.height.max(1),
            &filter_composite,
        );
        let scene_view = rtt_view.as_ref().unwrap_or(&view);

        {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Display Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: scene_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Store,
                    }),
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }

        with_tactical_view_ref(|tactical_view| {
            let camera_pos = tactical_view.get_3d_camera_position();
            let target = tactical_view.position();
            let eye = nalgebra::Point3::new(camera_pos.x, camera_pos.y, camera_pos.z);
            let target = nalgebra::Point3::new(target.x, target.y, target.z);
            let up = nalgebra::Vector3::new(0.0, 0.0, 1.0);

            let view_matrix = nalgebra::Matrix4::look_at_rh(&eye, &target, &up);
            let aspect =
                (tactical_view.width() as f32 / tactical_view.height().max(1) as f32).max(0.01);
            let projection_matrix = nalgebra::Matrix4::new_perspective(
                aspect,
                vertical_fov_from_horizontal(tactical_view.field_of_view(), aspect),
                1.0,
                20000.0,
            );

            let view_glam = glam::Mat4::from_cols_array_2d(&view_matrix.into());
            let proj_glam = glam::Mat4::from_cols_array_2d(&projection_matrix.into());

            // Terrain rendering pass: guard must outlive RenderPass due to wgpu borrow on record_chunk_draws.
            if let Ok(mut terrain_guard) = THE_TERRAIN_VISUAL.lock() {
                if let Some(terrain) = terrain_guard.as_mut() {
                    let _ = terrain.render(&view_glam, &proj_glam);
                    {
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Display Terrain Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: scene_view,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: Some(
                                wgpu::RenderPassDepthStencilAttachment {
                                    view: &self.depth_view,
                                    depth_ops: Some(wgpu::Operations {
                                        load: wgpu::LoadOp::Load,
                                        store: wgpu::StoreOp::Store,
                                    }),
                                    stencil_ops: Some(wgpu::Operations {
                                        load: wgpu::LoadOp::Load,
                                        store: wgpu::StoreOp::Store,
                                    }),
                                },
                            ),
                            occlusion_query_set: None,
                            timestamp_writes: None,
                        });
                        terrain.record_chunk_draws(&mut pass);
                    }
                }
            }

            // Drawable rendering pass: render units, buildings, and other game objects
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Display Drawable Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: scene_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                    }),
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                crate::drawable::drawable_manager::with_drawable_manager(|manager| {
                    manager.set_camera(
                        Self::nalgebra_to_game_matrix(&view_matrix),
                        Self::nalgebra_to_game_matrix(&projection_matrix),
                        crate::drawable::Vector3::new(camera_pos.x, camera_pos.y, camera_pos.z),
                    );
                    manager.cull_and_sort();
                    manager.render_pass_through(&mut pass, &view_glam, &proj_glam);
                });
            }

            // C++ DoShadows(true) after opaque flush, then
            // flushOccludedObjectsIntoStencil + renderStenciledPlayerColor.
            let light_pos = self
                .lighting_state
                .lights
                .first()
                .map(|light| light.position)
                .unwrap_or([0.0, 0.0, -1.0]);
            shadow_pass::record_shadow_and_occlusion_passes(
                self.graphics.device(),
                &mut encoder,
                scene_view,
                &self.depth_view,
                proj_glam * view_glam,
                [camera_pos.x, camera_pos.y, camera_pos.z],
                light_pos,
                self.graphics.config().format,
                wgpu::TextureFormat::Depth32Float,
            );
        });

        // C++ W3DParticleSys::doParticles — terrain visible-box cull.
        if let Ok(mut guard) = crate::effects::particle_manager::get_particle_system_manager_mut() {
            if let Some(mgr) = guard.as_mut() {
                with_tactical_view_ref(|view| {
                    let cam = view.get_3d_camera_position();
                    let target = view.position();
                    let aspect = (view.width() as f32 / view.height().max(1) as f32).max(0.01);
                    let visible = shadow_pass::maximum_visible_box(
                        [cam.x, cam.y, cam.z],
                        [target.x, target.y, target.z],
                        1.0,
                        20000.0,
                        vertical_fov_from_horizontal(view.field_of_view(), aspect),
                        aspect,
                        shadow_pass::terrain_min_height(),
                    );
                    mgr.cull_particles_to_visible_box(visible.center, visible.extent, 512);
                });
            }
        }

        // Particle rendering via W3DParticleSystemBridge
        #[cfg(feature = "w3d_support")]
        if let Some(renderer) = self.particle_renderer.as_ref() {
            if let Ok(manager_guard) = get_particle_system_manager() {
                if let Some(manager) = manager_guard.as_ref() {
                    let mut uniforms = self.build_particle_uniforms();
                    let particle_count: usize = manager
                        .all_particle_systems()
                        .map(|s| s.particle_count())
                        .sum();
                    uniforms.particle_count = particle_count as u32;
                    if let Ok(mut renderer_guard) = renderer.lock() {
                        if let Ok(mut bridge) = self.particle_bridge.lock() {
                            bridge.do_particles(
                                manager,
                                &mut *renderer_guard,
                                &mut encoder,
                                scene_view,
                                &self.depth_view,
                                &uniforms,
                            );
                        }
                    }
                }
            }
        }

        #[cfg(not(feature = "w3d_support"))]
        if let Some(renderer) = self.particle_renderer.as_ref() {
            if let Ok(manager_guard) = get_particle_system_manager() {
                if let Some(manager) = manager_guard.as_ref() {
                    let systems: Vec<_> = manager.all_particle_systems().collect();
                    if !systems.is_empty() {
                        let mut uniforms = self.build_particle_uniforms();
                        let particle_count: usize =
                            systems.iter().map(|system| system.particle_count()).sum();
                        uniforms.particle_count = particle_count as u32;
                        if let Ok(mut renderer_guard) = renderer.lock() {
                            renderer_guard.render_particles(
                                &mut encoder,
                                scene_view,
                                &self.depth_view,
                                &systems,
                                &uniforms,
                            );
                        }
                    }
                }
            }
            let uniforms = self.build_particle_uniforms();
            if let Ok(mut renderer_guard) = renderer.lock() {
                renderer_guard.render_tracer_and_ray_fx(
                    &mut encoder,
                    scene_view,
                    &self.depth_view,
                    &uniforms,
                );
            }
        }

        // Weather and decal rendering pass
        if let Some(renderer) = self.particle_renderer.as_ref() {
            if let Ok(weather_guard) = get_weather_system() {
                if let Some(weather) = weather_guard.as_ref() {
                    let particles = weather.get_all_particles();
                    if !particles.is_empty() {
                        let mut uniforms = self.build_particle_uniforms();
                        uniforms.particle_count = particles.len() as u32;
                        if let Ok(mut renderer_guard) = renderer.lock() {
                            renderer_guard.render_weather_particles(
                                &mut encoder,
                                scene_view,
                                &self.depth_view,
                                &particles,
                                &uniforms,
                            );
                        }
                    }
                }
            }
            let mut decals = if let Some(manager) = get_decal_manager() {
                manager
                    .lock()
                    .map(|guard| guard.collect_render_items())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            // C++ W3DProjectedShadowManager::flushDecals — unit + radius decals.
            decals.extend(shadow_pass::collect_unit_decal_items());

            if !decals.is_empty() {
                let mut uniforms = self.build_particle_uniforms();
                uniforms.particle_count = decals.len() as u32;
                if let Ok(mut renderer_guard) = renderer.lock() {
                    renderer_guard.render_decals(
                        &mut encoder,
                        scene_view,
                        &self.depth_view,
                        &decals,
                        &uniforms,
                    );
                }
            }
        }
        if rtt_view.is_some() {
            crate::display::shader_filter::filter_post_render(
                self.graphics.device(),
                self.graphics.queue(),
                &mut encoder,
                &view,
                &filter_composite,
            );
        }

        // UI rendering pass
        let ui_result =
            with_ui_renderer(|renderer_arc| -> Result<(), Box<dyn std::error::Error>> {
                {
                    let mut renderer = renderer_arc
                        .write()
                        .map_err(|_| "UI renderer lock poisoned")?;
                    renderer.begin_frame();
                    renderer.set_time(self.start_time.elapsed().as_secs_f32());
                    renderer.set_screen_size(self.width.max(1), self.height.max(1));
                }
                // Drop the write guard before gadget draw so `with_ui_renderer_mut`
                // can record real commands. Holding it discarded WND draws.
                with_window_manager(|manager| manager.draw_all());
                if display_fx::present_movie_overlay(self.width, self.height) {
                    let _ = display_fx::present_copyright_overlay(self.width, self.height);
                }
                if self.debug_display_callback.is_some() {
                    let fps = display_fx::note_frame_for_fps();
                    let particles = get_particle_system_manager()
                        .ok()
                        .and_then(|g| g.as_ref().map(|m| m.particle_count() as u32))
                        .unwrap_or(0);
                    display_fx::draw_debug_overlay(self.width, fps, 0, particles);
                    display_fx::draw_framerate_bar(self.width, fps, 30.0);
                }
                let mut renderer = renderer_arc
                    .write()
                    .map_err(|_| "UI renderer lock poisoned")?;
                self.queue_letterbox_and_filters(&mut renderer);
                let render_result = {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Display UI Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                    });
                    renderer.render(&mut render_pass)
                };
                renderer.end_frame();
                render_result.map_err(|err| Box::new(err) as Box<dyn std::error::Error>)
            });
        if let Some(Err(err)) = ui_result {
            return Err(err);
        }

        display_fx::apply_gamma_pass(
            &mut encoder,
            self.graphics.device(),
            self.graphics.config().format,
            &view,
            self.width.max(1),
            self.height.max(1),
        );
        self.graphics
            .queue()
            .submit(std::iter::once(encoder.finish()));

        // C++ WW3D::Toggle_Movie_Capture / takeScreenShot dump the presented
        // framebuffer, never the decompressed Bink movie buffer.
        if self.movie_capture_enabled || display_fx::is_movie_capture_enabled() {
            let _ = display_fx::write_backbuffer_movie_frame();
        }
        if let Ok(mut pending) = self.pending_screenshot.lock() {
            if let Some(path) = pending.take() {
                if !display_fx::write_backbuffer_screenshot(&path) {
                    display_fx::queue_screenshot(path);
                }
            }
        }
        if let Some(path) = display_fx::take_pending_screenshot() {
            let _ = display_fx::write_backbuffer_screenshot(&path);
        }

        frame.present();
        Ok(())
    }

    fn preload_common_textures(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::ini::ini_game_data::{Coord3D, RGBColor};

    #[test]
    fn display_lighting_uses_object_lighting_for_scene_and_w3d_lights() {
        let mut global_data = GlobalData::new();
        global_data.init();
        global_data.num_global_lights = 2;
        let tod = TimeOfDay::Night;
        let index = time_of_day_index(tod);

        global_data.terrain_objects_lighting[index][0].ambient = RGBColor::new(0.1, 0.2, 0.3);
        global_data.terrain_objects_lighting[index][0].diffuse = RGBColor::new(0.4, 0.5, 0.6);
        global_data.terrain_objects_lighting[index][0].light_pos = Coord3D::new(1.0, 2.0, 3.0);
        global_data.terrain_objects_lighting[index][1].ambient = RGBColor::new(0.7, 0.8, 0.9);
        global_data.terrain_objects_lighting[index][1].diffuse = RGBColor::new(0.11, 0.12, 0.13);
        global_data.terrain_objects_lighting[index][1].light_pos = Coord3D::new(4.0, 5.0, 6.0);

        let state = DisplayLightingState::from_global_data(&global_data, tod, true);

        assert_eq!(state.time_of_day, TimeOfDay::Night);
        assert_eq!(state.scene_ambient, [0.1, 0.2, 0.3]);
        assert_eq!(state.active_light_count, 2);
        assert_eq!(state.lights[0].ambient, [0.0, 0.0, 0.0]);
        assert_eq!(state.lights[0].diffuse, [0.4, 0.5, 0.6]);
        assert_eq!(state.lights[0].specular, [0.0, 0.0, 0.0]);
        assert_eq!(state.lights[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(state.lights[1].ambient, [0.0, 0.0, 0.0]);
        assert_eq!(state.lights[1].diffuse, [0.11, 0.12, 0.13]);
        assert_eq!(state.lights[1].position, [4.0, 5.0, 6.0]);
        assert_eq!(state.terrain_time_of_day, TimeOfDay::Night);
        assert!(state.tactical_view_redraw_forced);
    }

    #[test]
    fn display_lighting_clamps_active_light_count_and_invalid_uses_current_time() {
        let mut global_data = GlobalData::new();
        global_data.init();
        global_data.time_of_day = TimeOfDay::Evening;
        global_data.num_global_lights = MAX_GLOBAL_LIGHTS as i32 + 7;
        let index = time_of_day_index(TimeOfDay::Evening);
        global_data.terrain_objects_lighting[index][0].ambient = RGBColor::new(0.2, 0.3, 0.4);

        let state = DisplayLightingState::from_global_data(&global_data, TimeOfDay::Invalid, false);

        assert_eq!(state.time_of_day, TimeOfDay::Evening);
        assert_eq!(state.scene_ambient, [0.2, 0.3, 0.4]);
        assert_eq!(state.active_light_count, MAX_GLOBAL_LIGHTS);
        assert!(!state.tactical_view_redraw_forced);
    }

    #[test]
    fn set_display_mode_rejects_zero_resolution() {
        // C++ W3DDisplay::setDisplayMode returns FALSE and restores on device failure.
        assert!(!display_fx::is_valid_device_mode(0, 0, 32));
        assert!(display_fx::display_mode_count() > 0);
        let (w, h, depth) = display_fx::display_mode_description(0).unwrap();
        assert!(w >= 800 && h >= 600 && depth >= 24);
    }

    #[test]
    fn leftover_letterbox_matches_cpp_fade_out() {
        let fade_out = display_fx::letterbox_plan(1280.0, 1024.0, 0.4, false);
        assert!(fade_out.draw_top);
        assert!(!fade_out.draw_bottom);
    }
}
