//! Core application framework for game development tools

use crate::chrome::Chrome;
use crate::{GameTool, ThemeType, ToolConfig, UIError};
use anyhow::Result;
use eframe::egui;
use log::{error, info, warn};
use parking_lot::RwLock;
use std::sync::Arc;

/// Modern application framework for game development tools
pub struct ToolApp {
    tool: Box<dyn GameTool + Send + Sync>,
    config: ToolConfig,
    theme_manager: ThemeManager,
    hot_reload: Arc<RwLock<crate::hot_reload::HotReloadManager>>,
    performance_monitor: PerformanceMonitor,
    chrome: Chrome,
}

impl ToolApp {
    /// Create a new tool application
    pub fn new(tool: Box<dyn GameTool + Send + Sync>) -> Result<Self> {
        let config = tool.config().clone();
        let theme_manager = ThemeManager::new(config.theme);
        let hot_reload = Arc::new(RwLock::new(crate::hot_reload::HotReloadManager::new(
            config.hot_reload_enabled,
        )?));

        info!("Initializing tool: {} v{}", config.name, config.version);

        Ok(Self {
            tool,
            config,
            theme_manager,
            hot_reload,
            performance_monitor: PerformanceMonitor::new(),
            chrome: Chrome::new(),
        })
    }

    /// Shared chrome (menu bar, palette, status, dock layout).
    pub fn chrome(&self) -> &Chrome {
        &self.chrome
    }

    /// Mutable chrome for tools that need to update status/selection.
    pub fn chrome_mut(&mut self) -> &mut Chrome {
        &mut self.chrome
    }

    /// Run the application
    pub fn run(mut self) -> Result<()> {
        let mut viewport_builder = egui::ViewportBuilder::default()
            .with_inner_size([self.config.window_size[0], self.config.window_size[1]])
            .with_resizable(true)
            .with_decorations(true)
            .with_drag_and_drop(true);

        if let Some(pos) = self.config.window_position {
            viewport_builder = viewport_builder.with_position([pos[0], pos[1]]);
        }

        let options = eframe::NativeOptions {
            viewport: viewport_builder,
            ..Default::default()
        };

        let app_name = self.config.name.clone();

        eframe::run_native(
            &app_name,
            options,
            Box::new(move |cc| {
                // Configure graphics
                if let Some(_gl) = &cc.gl {
                    // Initialize 3D rendering context
                    info!("OpenGL context initialized");
                }

                // Apply theme
                self.theme_manager.apply_theme(&cc.egui_ctx);

                // Initialize tool
                if let Err(e) = self.tool.initialize() {
                    error!("Failed to initialize tool: {}", e);
                }

                Ok(Box::new(self))
            }),
        )
        .map_err(|e| UIError::WindowCreationFailed(e.to_string()))?;

        Ok(())
    }
}

impl eframe::App for ToolApp {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        self.performance_monitor.frame_start();

        // Check for hot reload updates
        if self.config.hot_reload_enabled {
            if let Some(mut hot_reload) = self.hot_reload.try_write() {
                if hot_reload.check_for_changes() {
                    info!("Hot reload triggered");
                    ctx.request_repaint();
                }
            }
        }

        self.sync_chrome_view_state();

        let fps = self.performance_monitor.get_fps();
        let mem_usage = self.performance_monitor.get_memory_usage();
        let command = self.chrome.show_shell(
            ctx,
            |ui| {
                if let Err(e) = self.tool.menu_bar(ui) {
                    warn!("Tool menu error: {}", e);
                }
            },
            |ui| {
                ui.label(format!("FPS: {:.0}", fps));
            },
            |ui| {
                ui.label(format!("Memory: {:.1} MB", mem_usage));
            },
        );

        if let Some(cmd) = command {
            self.handle_chrome_command(ctx, &cmd);
        }

        // Center viewport: remaining space after chrome docks.
        if let Err(e) = self.tool.update(ctx, frame) {
            error!("Tool update error: {}", e);
        }

        self.performance_monitor.frame_end();

        // Check for close event and handle shutdown
        ctx.input(|i| {
            if i.viewport().close_requested() {
                if let Err(e) = self.tool.shutdown() {
                    error!("Tool shutdown error: {}", e);
                }
                info!("Shutting down tool: {}", self.config.name);
            }
        });
    }
}

impl ToolApp {
    fn sync_chrome_view_state(&mut self) {
        let theme = self.config.theme;
        self.chrome
            .menu_bar
            .set_item_checked("view.theme.dark", theme == ThemeType::Dark);
        self.chrome
            .menu_bar
            .set_item_checked("view.theme.light", theme == ThemeType::Light);
        self.chrome
            .menu_bar
            .set_item_checked("view.theme.cnc_classic", theme == ThemeType::CnCClassic);
        self.chrome
            .menu_bar
            .set_item_checked("view.theme.modern", theme == ThemeType::Modern);
        self.chrome
            .menu_bar
            .set_item_checked("view.hot_reload", self.config.hot_reload_enabled);
        self.chrome
            .menu_bar
            .set_item_checked("view.palette", self.chrome.layout.show_left_palette);
        self.chrome
            .menu_bar
            .set_item_checked("view.properties", self.chrome.layout.show_right_properties);
    }

    fn handle_chrome_command(&mut self, ctx: &egui::Context, cmd: &str) {
        match cmd {
            "file.exit" => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            "view.theme.dark" => self.apply_theme(ctx, ThemeType::Dark),
            "view.theme.light" => self.apply_theme(ctx, ThemeType::Light),
            "view.theme.cnc_classic" => self.apply_theme(ctx, ThemeType::CnCClassic),
            "view.theme.modern" => self.apply_theme(ctx, ThemeType::Modern),
            "view.hot_reload" => {
                self.config.hot_reload_enabled = !self.config.hot_reload_enabled;
            }
            "view.palette" => {
                self.chrome.layout.show_left_palette = !self.chrome.layout.show_left_palette;
            }
            "view.properties" => {
                self.chrome.layout.show_right_properties =
                    !self.chrome.layout.show_right_properties;
            }
            "view.zoom_in" => {
                let zoom = self.chrome.status_bar.zoom();
                self.chrome.status_bar.set_zoom(zoom * 1.25);
            }
            "view.zoom_out" => {
                let zoom = self.chrome.status_bar.zoom();
                self.chrome.status_bar.set_zoom(zoom / 1.25);
            }
            "view.zoom_reset" => {
                self.chrome.status_bar.set_zoom(1.0);
            }
            "help.about" => {
                self.chrome
                    .status_bar
                    .set_message(format!("{} v{}", self.config.name, self.config.version));
            }
            _ => {}
        }
    }

    fn apply_theme(&mut self, ctx: &egui::Context, theme: ThemeType) {
        self.config.theme = theme;
        self.theme_manager.set_theme(theme);
        self.theme_manager.apply_theme(ctx);
    }
}

/// Theme management for consistent UI styling
pub struct ThemeManager {
    current_theme: ThemeType,
}

impl ThemeManager {
    pub fn new(theme: ThemeType) -> Self {
        Self {
            current_theme: theme,
        }
    }

    pub fn set_theme(&mut self, theme: ThemeType) {
        self.current_theme = theme;
    }

    pub fn apply_theme(&self, ctx: &egui::Context) {
        let visuals = match self.current_theme {
            ThemeType::Dark => egui::Visuals::dark(),
            ThemeType::Light => egui::Visuals::light(),
            ThemeType::CnCClassic => self.cnc_classic_theme(),
            ThemeType::Modern => self.modern_theme(),
        };

        ctx.set_visuals(visuals);
    }

    fn cnc_classic_theme(&self) -> egui::Visuals {
        let mut visuals = egui::Visuals::dark();

        // Command & Conquer classic green/amber color scheme
        visuals.override_text_color = Some(egui::Color32::from_rgb(0, 255, 0));
        visuals.panel_fill = egui::Color32::from_gray(20);
        visuals.window_fill = egui::Color32::from_gray(25);
        visuals.extreme_bg_color = egui::Color32::BLACK;

        visuals
    }

    fn modern_theme(&self) -> egui::Visuals {
        let mut visuals = egui::Visuals::dark();

        // Modern dark theme with blue accents
        visuals.selection.bg_fill = egui::Color32::from_rgb(30, 100, 200);
        visuals.hyperlink_color = egui::Color32::from_rgb(100, 150, 255);

        visuals
    }
}

/// Performance monitoring for development tools
pub struct PerformanceMonitor {
    frame_times: Vec<f64>,
    frame_start_time: std::time::Instant,
    last_memory_check: std::time::Instant,
    memory_usage: f64,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            frame_times: Vec::with_capacity(60),
            frame_start_time: std::time::Instant::now(),
            last_memory_check: std::time::Instant::now(),
            memory_usage: 0.0,
        }
    }

    pub fn frame_start(&mut self) {
        self.frame_start_time = std::time::Instant::now();
    }

    pub fn frame_end(&mut self) {
        let frame_time = self.frame_start_time.elapsed().as_secs_f64();

        self.frame_times.push(frame_time);
        if self.frame_times.len() > 60 {
            self.frame_times.remove(0);
        }

        // Update memory usage every second
        if self.last_memory_check.elapsed().as_secs() >= 1 {
            self.update_memory_usage();
            self.last_memory_check = std::time::Instant::now();
        }
    }

    pub fn get_fps(&self) -> f64 {
        if self.frame_times.is_empty() {
            return 0.0;
        }

        let avg_frame_time: f64 =
            self.frame_times.iter().sum::<f64>() / self.frame_times.len() as f64;
        1.0 / avg_frame_time
    }

    pub fn get_memory_usage(&self) -> f64 {
        self.memory_usage
    }

    fn update_memory_usage(&mut self) {
        // Simple memory usage estimation
        // In a real implementation, you'd use platform-specific APIs
        self.memory_usage = 0.0; // Placeholder
    }
}
