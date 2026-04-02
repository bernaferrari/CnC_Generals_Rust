//! Complete UI System Integration
//!
//! This module provides a complete, integrated UI system that matches the original
//! Command & Conquer Generals exactly. It combines all the enhanced components
//! (renderer, window manager, shell, control bar, gadgets) into a unified system.

use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;
use thiserror::Error;
use wgpu::{Device, Queue, TextureFormat, TextureView};
use winit::event::WindowEvent;

use crate::core::subsystems::{
    CommandLogEntry, InGameUISubsystem, RadarPingEvent, RadarPingKind, SelectionEvent,
};
use crate::display::image::get_mapped_image_collection;
use crate::helpers::{register_control_bar_backend, ControlBarHooks};
use gamelogic::control_bar::{register_control_bar_ui_hooks, ControlBarUiHooks};
use game_engine::common::SubsystemInterface;

use super::{
    ControlBarError, EnhancedControlBar, EnhancedGameWindow, EnhancedShell, EnhancedWindowManager,
    FontError, FontLibrary, GadgetError, GadgetManager, ShellError, UIRenderer, UIRendererError,
    WindowManagerError, WindowStatus,
};

/// Complete UI system errors
#[derive(Error, Debug)]
pub enum CompleteUIError {
    #[error("UI renderer error: {0}")]
    RendererError(#[from] UIRendererError),
    #[error("Window manager error: {0}")]
    WindowManagerError(#[from] WindowManagerError),
    #[error("Shell error: {0}")]
    ShellError(#[from] ShellError),
    #[error("Control bar error: {0}")]
    ControlBarError(#[from] ControlBarError),
    #[error("Gadget error: {0}")]
    GadgetError(#[from] GadgetError),
    #[error("Font error: {0}")]
    FontError(#[from] FontError),
    #[error("System not initialized")]
    NotInitialized,
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
}

type Result<T> = std::result::Result<T, CompleteUIError>;

/// UI System Configuration
#[derive(Debug, Clone)]
pub struct UISystemConfig {
    /// Screen resolution
    pub screen_width: u32,
    pub screen_height: u32,

    /// Texture format for rendering
    pub texture_format: TextureFormat,

    /// Enable anti-aliasing
    pub enable_msaa: bool,

    /// MSAA sample count
    pub msaa_samples: u32,

    /// Enable VSync
    pub enable_vsync: bool,

    /// UI scaling factor
    pub ui_scale: f32,

    /// Font configuration
    pub default_font: String,
    pub font_size: f32,

    /// Asset paths
    pub asset_root: String,
    pub font_path: String,
    pub image_path: String,
    pub layout_path: String,

    /// Performance settings
    pub max_draw_calls_per_frame: u32,
    pub enable_ui_batching: bool,
    pub enable_texture_atlas: bool,
}

impl Default for UISystemConfig {
    fn default() -> Self {
        Self {
            screen_width: 1024,
            screen_height: 768,
            texture_format: TextureFormat::Bgra8UnormSrgb,
            enable_msaa: true,
            msaa_samples: 4,
            enable_vsync: true,
            ui_scale: 1.0,
            default_font: "Arial".to_string(),
            font_size: 14.0,
            asset_root: "Data".to_string(),
            font_path: "Data/Fonts".to_string(),
            image_path: "Data/Images".to_string(),
            layout_path: "Data/Layouts".to_string(),
            max_draw_calls_per_frame: 1000,
            enable_ui_batching: true,
            enable_texture_atlas: true,
        }
    }
}

/// Complete UI System - integrates all components
pub struct CompleteUISystem {
    // Configuration
    config: UISystemConfig,
    initialized: bool,

    // Core rendering
    renderer: Arc<RwLock<UIRenderer>>,

    // Window management
    window_manager: Arc<EnhancedWindowManager>,

    // Systems
    shell: EnhancedShell,
    control_bar: Arc<Mutex<EnhancedControlBar>>,
    gadget_manager: GadgetManager,
    font_library: FontLibrary,
    in_game_ui: Option<Arc<Mutex<InGameUISubsystem>>>,
    radar_pings: Vec<RadarPingEvent>,

    // State tracking
    current_time: f32,
    frame_count: u64,
    last_update_time: Instant,

    // Performance metrics
    frame_time_ms: f32,
    render_time_ms: f32,
    update_time_ms: f32,
}

impl CompleteUISystem {
    /// Create a new complete UI system
    pub fn new(config: UISystemConfig) -> Self {
        Self {
            config,
            initialized: false,
            renderer: Arc::new(RwLock::new(unsafe { std::mem::zeroed() })), // Placeholder
            window_manager: Arc::new(EnhancedWindowManager::new()),
            shell: EnhancedShell::new(),
            control_bar: Arc::new(Mutex::new(EnhancedControlBar::new())),
            gadget_manager: GadgetManager::new(),
            font_library: FontLibrary::new(),
            in_game_ui: None,
            radar_pings: Vec::new(),
            current_time: 0.0,
            frame_count: 0,
            last_update_time: Instant::now(),
            frame_time_ms: 0.0,
            render_time_ms: 0.0,
            update_time_ms: 0.0,
        }
    }

    /// Initialize the complete UI system
    pub fn initialize(&mut self, device: Arc<Device>, queue: Arc<Queue>) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        let start_time = Instant::now();

        // Initialize UI renderer
        let renderer = UIRenderer::new(device.clone(), queue.clone(), self.config.texture_format)?;
        self.renderer = Arc::new(RwLock::new(renderer));
        crate::gui::set_ui_renderer(self.renderer.clone());

        // Configure renderer
        {
            let mut renderer = self.renderer.write().unwrap();
            renderer.set_screen_size(self.config.screen_width, self.config.screen_height);
            renderer.set_time(0.0);
        }

        // Initialize window manager
        self.window_manager.initialize(self.renderer.clone())?;

        // Initialize font library
        self.font_library.init()?;

        // Load default font
        // In a real implementation, you would load font data from file
        // let font_data = std::fs::read(format!("{}/{}.ttf", self.config.font_path, self.config.default_font))?;
        // self.renderer.write().unwrap().load_font(&self.config.default_font, &font_data)?;

        // Initialize shell system
        self.shell.set_window_manager(self.window_manager.clone());
        self.shell
            .init()
            .map_err(|e| CompleteUIError::ShellError(ShellError::LayoutError(e.to_string())))?;

        // Initialize control bar
        {
            let mut control_bar = self
                .control_bar
                .lock()
                .map_err(|_| CompleteUIError::ControlBarError(ControlBarError::InvalidContext(
                    "Control bar lock poisoned".to_string(),
                )))?;
            control_bar.set_window_manager(self.window_manager.clone());
            control_bar.init().map_err(|e| {
                CompleteUIError::ControlBarError(ControlBarError::WindowError(e.to_string()))
            })?;
        }
        register_control_bar_backend(Arc::new(ControlBarBackend::new(self.control_bar.clone())));
        register_control_bar_ui_hooks(Arc::new(ControlBarUiBackend::new(
            self.control_bar.clone(),
        )));

        // Load default UI scheme
        if let Err(e) = self.shell.load_scheme(
            "Default",
            &format!("{}/DefaultScheme.xml", self.config.asset_root),
        ) {
            log::warn!("Failed to load default UI scheme: {}", e);
        }

        if let Ok(mut control_bar) = self.control_bar.lock() {
            if let Err(e) = control_bar.set_control_bar_scheme_by_name("Default") {
                log::warn!("Failed to set default control bar scheme: {}", e);
            }
        }

        // Load UI images and upload GPU textures for rendering.
        {
            let collection = get_mapped_image_collection();
            let mut collection = collection.write();
            if let Err(err) = collection.load_from_directory(&self.config.image_path, true) {
                log::warn!("Failed to load UI images: {}", err);
            }
            let renderer = self.renderer.read().unwrap();
            if let Err(err) = collection.create_gpu_textures(renderer.device(), renderer.queue()) {
                log::warn!("Failed to upload UI textures: {}", err);
            }
        }

        // Create initial UI layout
        self.create_initial_layout()?;

        self.initialized = true;

        let init_time = start_time.elapsed().as_secs_f32() * 1000.0;
        log::info!("UI System initialized in {:.2}ms", init_time);

        Ok(())
    }

    /// Wire the in-game UI subsystem into the UI so control bar widgets can
    /// mirror live gameplay state.
    pub fn set_in_game_ui(&mut self, ui: Arc<Mutex<InGameUISubsystem>>) {
        if let Ok(mut control_bar) = self.control_bar.lock() {
            control_bar.set_in_game_ui_handle(ui.clone());
        }
        self.in_game_ui = Some(ui);
    }

    /// Returns the beacon panel lines currently built by the control bar.
    pub fn beacon_panel_lines(&self) -> &[String] {
        self.control_bar
            .lock()
            .map(|control_bar| control_bar.beacon_panel_lines())
            .unwrap_or(&[])
    }

    /// Returns the recent selection history captured from gameplay input.
    pub fn selection_history(&self) -> Vec<SelectionEvent> {
        self.control_bar
            .lock()
            .map(|control_bar| control_bar.selection_history())
            .unwrap_or_default()
    }

    /// Returns the current command log captured from gameplay input.
    pub fn command_history(&self) -> Vec<CommandLogEntry> {
        self.control_bar
            .lock()
            .map(|control_bar| control_bar.command_history())
            .unwrap_or_default()
    }

    /// Latest radar pings captured from the game this frame.
    pub fn radar_pings(&self) -> &[RadarPingEvent] {
        &self.radar_pings
    }

    /// Convert stored radar pings into normalized minimap dots for consumers.
    pub fn minimap_radar_dots(
        &self,
        world_min: glam::Vec3,
        world_max: glam::Vec3,
    ) -> Vec<(f32, f32, f32, RadarPingKind)> {
        let span_x = (world_max.x - world_min.x).abs().max(0.001);
        let span_z = (world_max.z - world_min.z).abs().max(0.001);
        self.radar_pings
            .iter()
            .map(|ping| {
                let nx = ((ping.position.x - world_min.x) / span_x).clamp(0.0, 1.0);
                let nz = ((ping.position.z - world_min.z) / span_z).clamp(0.0, 1.0);
                (nx, nz, ping.age_seconds, ping.kind)
            })
            .collect()
    }

    /// Shutdown the UI system
    pub fn shutdown(&mut self) -> Result<()> {
        if !self.initialized {
            return Ok(());
        }

        // Shutdown shell
        self.shell
            .reset()
            .map_err(|e| CompleteUIError::ShellError(ShellError::LayoutError(e.to_string())))?;

        // Shutdown control bar
        {
            let mut control_bar = self
                .control_bar
                .lock()
                .map_err(|_| CompleteUIError::ControlBarError(ControlBarError::InvalidContext(
                    "Control bar lock poisoned".to_string(),
                )))?;
            control_bar.reset().map_err(|e| {
                CompleteUIError::ControlBarError(ControlBarError::WindowError(e.to_string()))
            })?;
        }

        // Clear gadgets
        self.gadget_manager.clear();

        self.initialized = false;

        log::info!("UI System shutdown complete");
        Ok(())
    }

    /// Handle window events from winit
    pub fn handle_window_event(&mut self, event: &WindowEvent) -> Result<bool> {
        if !self.initialized {
            return Ok(false);
        }

        self.window_manager
            .handle_window_event(event)
            .map_err(|e| CompleteUIError::WindowManagerError(e))
    }

    /// Update the UI system (call once per frame)
    pub fn update(&mut self, delta_time: f32) -> Result<()> {
        if !self.initialized {
            return Err(CompleteUIError::NotInitialized);
        }

        let update_start = Instant::now();

        // Update time
        self.current_time += delta_time;
        self.frame_count += 1;

        // Update renderer time
        {
            let mut renderer = self.renderer.write().unwrap();
            renderer.set_time(self.current_time);
        }

        // Update window manager
        self.window_manager.update()?;

        // Update shell
        let duration = std::time::Duration::from_secs_f32(delta_time);
        self.shell
            .update(duration)
            .map_err(|e| CompleteUIError::ShellError(ShellError::LayoutError(e.to_string())))?;

        // Update control bar
        {
            let mut control_bar = self
                .control_bar
                .lock()
                .map_err(|_| CompleteUIError::ControlBarError(ControlBarError::InvalidContext(
                    "Control bar lock poisoned".to_string(),
                )))?;
            control_bar.update(duration).map_err(|e| {
                CompleteUIError::ControlBarError(ControlBarError::WindowError(e.to_string()))
            })?;
        }

        // Age existing radar pings so consumers can apply decay visualizations.
        for ping in &mut self.radar_pings {
            ping.age_seconds += delta_time;
        }

        // Drain radar pings buffered by the in-game UI so HUD/minimap layers can consume or log.
        if let Some(ui_handle) = &self.in_game_ui {
            if let Ok(mut ui) = ui_handle.lock() {
                self.radar_pings.extend(ui.drain_radar_pings());
            }
        }

        // Cull stale/overflow pings to mirror C++ radar decay.
        const RADAR_PING_TTL: f32 = 6.0;
        self.radar_pings
            .retain(|ping| ping.age_seconds <= RADAR_PING_TTL);
        if self.radar_pings.len() > 48 {
            // Keep most recent by age
            self.radar_pings.sort_by(|a, b| a.age_seconds.partial_cmp(&b.age_seconds).unwrap());
            self.radar_pings.truncate(48);
        }

        // Update gadgets
        self.gadget_manager.update(delta_time)?;

        self.update_time_ms = update_start.elapsed().as_secs_f32() * 1000.0;

        Ok(())
    }

    /// Render the UI system
    pub fn render(&mut self, target_view: &TextureView) -> Result<()> {
        if !self.initialized {
            return Err(CompleteUIError::NotInitialized);
        }

        let render_start = Instant::now();

        // Create render pass
        let mut encoder = self
            .renderer
            .read()
            .unwrap()
            .device()
            .create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label: Some("UI Render Encoder"),
            },
        );

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Render UI on top of the game target
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Feed radar overlay to the window manager so it can draw the minimap inset.
            self.window_manager
                .set_radar_overlay(self.minimap_radar_dots(glam::Vec3::ZERO, glam::Vec3::new(1.0, 0.0, 1.0)));

            // Render window manager (includes all windows)
            self.window_manager.render()?;

            // Render UI through the renderer including any radar overlay emitted this frame.
            let mut renderer = self.renderer.write().unwrap();
            renderer.render(&mut render_pass)?;
        }

        // Submit the command buffer
        self.renderer
            .read()
            .unwrap()
            .queue()
            .submit(std::iter::once(encoder.finish()));

        self.render_time_ms = render_start.elapsed().as_secs_f32() * 1000.0;

        // Calculate frame time
        let now = Instant::now();
        self.frame_time_ms = now.duration_since(self.last_update_time).as_secs_f32() * 1000.0;
        self.last_update_time = now;

        Ok(())
    }

    /// Get the window manager
    pub fn get_window_manager(&self) -> &Arc<EnhancedWindowManager> {
        &self.window_manager
    }

    /// Get the shell system
    pub fn get_shell(&mut self) -> &mut EnhancedShell {
        &mut self.shell
    }

    /// Get the control bar
    pub fn get_control_bar(&mut self) -> Arc<Mutex<EnhancedControlBar>> {
        self.control_bar.clone()
    }

    /// Get the gadget manager
    pub fn get_gadget_manager(&mut self) -> &mut GadgetManager {
        &mut self.gadget_manager
    }

    /// Get the UI renderer (read-only)
    pub fn get_renderer(&self) -> &Arc<RwLock<UIRenderer>> {
        &self.renderer
    }

    /// Get performance statistics
    pub fn get_performance_stats(&self) -> UIPerformanceStats {
        let render_stats = self.renderer.read().unwrap().get_stats().clone();

        UIPerformanceStats {
            frame_time_ms: self.frame_time_ms,
            update_time_ms: self.update_time_ms,
            render_time_ms: self.render_time_ms,
            frame_count: self.frame_count,
            draw_calls: render_stats.draw_calls,
            vertices_rendered: render_stats.vertices_rendered,
            triangles_rendered: render_stats.triangles_rendered,
            texture_switches: render_stats.texture_switches,
        }
    }

    /// Set UI scaling factor
    pub fn set_ui_scale(&mut self, scale: f32) {
        self.config.ui_scale = scale;
        // Would apply scaling to all UI elements
    }

    /// Push a new menu screen
    pub fn push_menu(&mut self, menu_name: &str) -> Result<()> {
        let layout_path = format!("{}/{}.wnd", self.config.layout_path, menu_name);
        self.shell.push(&layout_path, false)?;
        Ok(())
    }

    /// Pop the current menu screen
    pub fn pop_menu(&mut self) -> Result<()> {
        self.shell.pop()?;
        Ok(())
    }

    /// Show the main menu
    pub fn show_main_menu(&mut self) -> Result<()> {
        self.push_menu("MainMenu")
    }

    /// Show the in-game HUD
    pub fn show_ingame_hud(&mut self) -> Result<()> {
        // Hide shell menus
        self.shell.hide(true);

        // Position the beacon panel relative to current resolution
        let panel_width = 320;
        let panel_height = 180;
        let margin = 24;
        let x = margin;
        let y = self.config.screen_height as i32 - panel_height - margin;
        if let Ok(mut control_bar) = self.control_bar.lock() {
            control_bar.set_beacon_panel_bounds(x, y, panel_width, panel_height);
        }

        // Initialize control bar for in-game
        if let Ok(mut control_bar) = self.control_bar.lock() {
            control_bar.mark_ui_dirty();
        }

        Ok(())
    }

    /// Hide the in-game HUD
    pub fn hide_ingame_hud(&mut self) -> Result<()> {
        // Show shell menus
        self.shell.hide(false);

        Ok(())
    }

    /// Create a simple dialog window
    pub fn create_dialog(
        &mut self,
        title: &str,
        message: &str,
        buttons: &[&str],
    ) -> Result<Arc<EnhancedGameWindow>> {
        let dialog = self.window_manager.create_window(
            None,
            title,
            (self.config.screen_width as i32 - 400) / 2,
            (self.config.screen_height as i32 - 200) / 2,
            400,
            200,
        )?;

        dialog.set_text(message);
        dialog.set_status(WindowStatus::ENABLED | WindowStatus::ABOVE);

        // Create buttons (simplified - real implementation would use gadget system)
        let button_width = 80;
        let button_spacing = 10;
        let total_button_width =
            buttons.len() as i32 * button_width + (buttons.len() as i32 - 1) * button_spacing;
        let start_x = (400 - total_button_width) / 2;

        for (i, &button_text) in buttons.iter().enumerate() {
            let button_x = start_x + i as i32 * (button_width + button_spacing);
            let button = self.window_manager.create_window(
                Some(&dialog),
                &format!("DialogButton{}", i),
                button_x,
                150,
                button_width,
                30,
            )?;

            button.set_text(button_text);
            button.set_status(WindowStatus::ENABLED);
        }

        // Show as modal
        self.window_manager
            .show_modal(dialog.get_id(), [0.0, 0.0, 0.0, 0.5], true)?;

        Ok(dialog)
    }

    // Private implementation methods

    fn create_initial_layout(&mut self) -> Result<()> {
        // Create the root UI layout

        // This would typically load from a layout file, but for now create programmatically
        let root_window = self.window_manager.create_window(
            None,
            "RootUI",
            0,
            0,
            self.config.screen_width as i32,
            self.config.screen_height as i32,
        )?;

        root_window.set_status(WindowStatus::ENABLED);

        // Create basic menu structure
        self.create_main_menu_layout()?;

        Ok(())
    }

    fn create_main_menu_layout(&mut self) -> Result<()> {
        // This would normally be loaded from a .wnd file
        // For demonstration, create a simple main menu

        let main_menu = self.window_manager.create_window(
            None,
            "MainMenu",
            0,
            0,
            self.config.screen_width as i32,
            self.config.screen_height as i32,
        )?;

        main_menu.set_status(WindowStatus::ENABLED | WindowStatus::HIDDEN); // Start hidden

        // Create menu buttons
        let button_width = 200;
        let button_height = 40;
        let button_spacing = 10;
        let menu_items = ["Single Player", "Multiplayer", "Options", "Credits", "Exit"];

        let start_y = (self.config.screen_height as i32
            - (menu_items.len() as i32 * (button_height + button_spacing)))
            / 2;
        let button_x = (self.config.screen_width as i32 - button_width) / 2;

        for (i, &item_text) in menu_items.iter().enumerate() {
            let button_y = start_y + i as i32 * (button_height + button_spacing);

            let button = self.window_manager.create_window(
                Some(&main_menu),
                &format!("MainMenu_{}", item_text.replace(" ", "")),
                button_x,
                button_y,
                button_width,
                button_height,
            )?;

            button.set_text(item_text);
            button.set_status(WindowStatus::ENABLED);
        }

        Ok(())
    }
}

struct ControlBarBackend {
    control_bar: Arc<Mutex<EnhancedControlBar>>,
}

impl ControlBarBackend {
    fn new(control_bar: Arc<Mutex<EnhancedControlBar>>) -> Self {
        Self { control_bar }
    }
}

struct ControlBarUiBackend {
    control_bar: Arc<Mutex<EnhancedControlBar>>,
}

impl ControlBarUiBackend {
    fn new(control_bar: Arc<Mutex<EnhancedControlBar>>) -> Self {
        Self { control_bar }
    }
}

impl ControlBarUiHooks for ControlBarUiBackend {
    fn mark_ui_dirty(&self) {
        if let Ok(mut control_bar) = self.control_bar.lock() {
            control_bar.mark_ui_dirty();
        }
    }

    fn on_player_science_purchase_points_changed(&self, player_id: i32, points: i32) {
        if let Ok(mut control_bar) = self.control_bar.lock() {
            control_bar.on_player_science_purchase_points_changed(player_id, points);
        }
    }

    fn on_player_rank_changed(&self, player_id: i32, rank_level: i32, points: i32) {
        if let Ok(mut control_bar) = self.control_bar.lock() {
            control_bar.on_player_rank_changed(player_id, rank_level, points);
        }
    }
}

impl ControlBarHooks for ControlBarBackend {
    fn hide_purchase_science(&self) {
        if let Ok(mut control_bar) = self.control_bar.lock() {
            control_bar.hide_purchase_science();
        }
    }

    fn process_context_sensitive_button_click(&self, control_id: u32, msg: u32) {
        if let Ok(mut control_bar) = self.control_bar.lock() {
            if let Err(err) = control_bar.process_context_sensitive_button_click_by_id(control_id, msg) {
                log::warn!("Control bar click handling failed: {}", err);
            }
        }
    }

    fn process_context_sensitive_button_transition(&self, control_id: u32, entering: bool) {
        if let Ok(mut control_bar) = self.control_bar.lock() {
            if let Err(err) =
                control_bar.process_context_sensitive_button_transition_by_id(control_id, entering)
            {
                log::warn!("Control bar transition handling failed: {}", err);
            }
        }
    }

    fn toggle_purchase_science(&self) {
        if let Ok(mut control_bar) = self.control_bar.lock() {
            control_bar.toggle_purchase_science();
        }
    }

    fn show_special_power_shortcut(&self) {
        if let Ok(mut control_bar) = self.control_bar.lock() {
            control_bar.show_special_power_shortcut();
        }
    }

    fn hide_special_power_shortcut(&self) {
        if let Ok(mut control_bar) = self.control_bar.lock() {
            control_bar.hide_special_power_shortcut();
        }
    }

    fn animate_special_power_shortcut(&self, enabled: bool) {
        if let Ok(mut control_bar) = self.control_bar.lock() {
            control_bar.animate_special_power_shortcut(enabled);
        }
    }

    fn init_special_power_shortcut_bar_for_player(&self, _player_side: &str) {
        if let Ok(mut control_bar) = self.control_bar.lock() {
            control_bar.show_special_power_shortcut();
        }
    }

    fn set_control_bar_scheme_by_player(&self, player_side: &str) {
        if let Ok(mut control_bar) = self.control_bar.lock() {
            if let Err(err) = control_bar.set_control_bar_scheme_by_player(player_side) {
                log::warn!("Failed to set control bar scheme for side '{}': {}", player_side, err);
            }
        }
    }

    fn toggle_control_bar_stage(&self) {
        if let Ok(mut control_bar) = self.control_bar.lock() {
            control_bar.toggle_control_bar_stage();
        }
    }

    fn get_observer_look_at_player_index(&self) -> Option<i32> {
        let control_bar = self.control_bar.lock().ok()?;
        control_bar.get_observer_look_at_player_index()
    }
}

/// UI Performance statistics
#[derive(Debug, Clone)]
pub struct UIPerformanceStats {
    pub frame_time_ms: f32,
    pub update_time_ms: f32,
    pub render_time_ms: f32,
    pub frame_count: u64,
    pub draw_calls: u32,
    pub vertices_rendered: u32,
    pub triangles_rendered: u32,
    pub texture_switches: u32,
}

impl UIPerformanceStats {
    pub fn fps(&self) -> f32 {
        if self.frame_time_ms > 0.0 {
            1000.0 / self.frame_time_ms
        } else {
            0.0
        }
    }

    pub fn total_time_ms(&self) -> f32 {
        self.update_time_ms + self.render_time_ms
    }

    pub fn efficiency(&self) -> f32 {
        if self.draw_calls > 0 {
            self.triangles_rendered as f32 / self.draw_calls as f32
        } else {
            0.0
        }
    }
}
