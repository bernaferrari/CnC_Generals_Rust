/*
**  Command & Conquer Generals Zero Hour™
*/

//! Display adaptor that renders through the shared `PlatformContext`.

use crate::display::view::{with_tactical_view, with_tactical_view_ref, ViewTrait};
use crate::display::DisplayInterface;
use crate::drawable::drawable_manager::DrawableManager;
use crate::effects::particle_manager::get_particle_system_manager;
use crate::effects::particle_renderer::{
    register_particle_renderer, ParticleRenderer as GpuParticleRenderer, ParticleUniforms,
};
use crate::effects::weather_complete::get_weather_system;
use crate::fx_list::get_decal_manager;
use crate::game_text::GameText;
use crate::gui::display_string::{get_display_string_manager, DisplayStringHandle};
use crate::gui::font::{get_font_library, FontDesc};
use crate::gui::{with_ui_renderer, with_window_manager};
use crate::platform::GraphicsContext;
use crate::system::debug_display::DebugDisplay;
use crate::system::SubsystemInterface;
use crate::terrain::terrain_visual::THE_TERRAIN_VISUAL;
use crate::terrain::TerrainVisual;
use crate::video_buffer::{SoftwareVideoBuffer, VideoBuffer, VideoBufferType};
use crate::video_player::{get_video_player, VideoPlayerInterface};
use crate::video_stream::VideoStreamInterface;
#[cfg(feature = "w3d_support")]
use crate::w3d::W3DParticleSystemBridge;
use gamelogic::helpers::TheGameLogic;
use log::{error, warn};
use nalgebra::{Matrix4, Point3, Vector3};
use std::any::Any;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wgpu::SurfaceTexture;

pub type DebugDisplayCallback = fn(&mut DebugDisplay, Option<&mut dyn Any>);

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
}

impl Display {
    pub fn new(graphics: GraphicsContext) -> Self {
        let (surface_format, width, height) = {
            let config = graphics.config();
            (config.format, config.width, config.height)
        };
        let depth_texture = Self::create_depth_texture(&graphics);
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let particle_renderer = match GpuParticleRenderer::new(
            graphics.device_arc(),
            graphics.queue_arc(),
            surface_format,
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
        }
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
    }

    pub fn set_height(&mut self, height: u32) {
        self.height = height;
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
        let old_display_width = self.get_width().max(1) as f32;
        let old_display_height = self.get_height().max(1) as f32;

        let (old_view_width, old_view_height, old_view_origin_x, old_view_origin_y) =
            with_tactical_view(|view| {
                (
                    view.width() as f32,
                    view.height() as f32,
                    view.origin().0 as f32,
                    view.origin().1 as f32,
                )
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

        true
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
        self.currently_playing_movie.clear();
        self.video_buffer = None;

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
    }

    pub fn is_movie_playing(&self) -> bool {
        self.video_stream.is_some() && self.video_buffer.is_some()
    }

    pub fn toggle_movie_capture(&mut self) {
        self.movie_capture_enabled = !self.movie_capture_enabled;
    }

    pub fn is_movie_capture_enabled(&self) -> bool {
        self.movie_capture_enabled
    }

    pub fn enable_letter_box(&mut self, enabled: bool) {
        self.letterbox_enabled = enabled;
        self.letterbox_fade_level = if enabled { 1.0 } else { 0.0 };
        self.letterbox_fade_start_time = Some(Instant::now());
    }

    pub fn is_letter_box_enabled(&self) -> bool {
        self.letterbox_enabled
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
        {
            let mut display_string_mut = display_string.borrow_mut();
            display_string_mut.set_text(GameText::fetch("GUI:EACopyright"));
            let font_desc = FontDesc::new("Courier", 12, true);
            let mut font_library = get_font_library();
            if let Ok(font) = font_library.get_font(&font_desc) {
                display_string_mut.set_font(font);
            }
        }
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
            let projection_matrix =
                Matrix4::new_perspective(aspect, view.field_of_view(), 1.0, 20000.0);

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

        {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Display Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
                    stencil_ops: None,
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
                tactical_view.field_of_view(),
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
                                view: &view,
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
                                    stencil_ops: None,
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
                        view: &view,
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
                        stencil_ops: None,
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
        });

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
                                &view,
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
                                &view,
                                &self.depth_view,
                                &systems,
                                &uniforms,
                            );
                        }
                    }
                }
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
                                &view,
                                &self.depth_view,
                                &particles,
                                &uniforms,
                            );
                        }
                    }
                }
            }
            if let Some(manager) = get_decal_manager() {
                if let Ok(guard) = manager.lock() {
                    let decals = guard.collect_render_items();
                    if !decals.is_empty() {
                        let mut uniforms = self.build_particle_uniforms();
                        uniforms.particle_count = decals.len() as u32;
                        if let Ok(mut renderer_guard) = renderer.lock() {
                            renderer_guard.render_decals(
                                &mut encoder,
                                &view,
                                &self.depth_view,
                                &decals,
                                &uniforms,
                            );
                        }
                    }
                }
            }
        }

        // UI rendering pass
        let ui_result = with_ui_renderer(|renderer| {
            let mut renderer = renderer.write().unwrap_or_else(|e| e.into_inner());
            renderer.begin_frame();
            renderer.set_time(self.start_time.elapsed().as_secs_f32());
            renderer.set_screen_size(self.width.max(1), self.height.max(1));
            with_window_manager(|manager| manager.draw_all());
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
            render_result
        });
        if let Some(Err(err)) = ui_result {
            return Err(Box::new(err));
        }

        self.graphics
            .queue()
            .submit(std::iter::once(encoder.finish()));

        frame.present();
        Ok(())
    }

    fn preload_common_textures(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
