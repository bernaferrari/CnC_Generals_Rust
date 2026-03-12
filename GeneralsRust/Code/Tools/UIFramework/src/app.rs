//! Core application framework for game development tools

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
        })
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

        // Main menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // File menu
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() {
                        // Handle new file
                    }
                    if ui.button("Open...").clicked() {
                        // Handle open file
                    }
                    if ui.button("Save").clicked() {
                        // Handle save
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                // Edit menu
                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo").clicked() {
                        // Handle undo
                    }
                    if ui.button("Redo").clicked() {
                        // Handle redo
                    }
                    ui.separator();
                    if ui.button("Preferences...").clicked() {
                        // Handle preferences
                    }
                });

                // View menu
                ui.menu_button("View", |ui| {
                    ui.menu_button("Theme", |ui| {
                        for theme in [
                            ThemeType::Dark,
                            ThemeType::Light,
                            ThemeType::CnCClassic,
                            ThemeType::Modern,
                        ] {
                            if ui
                                .selectable_label(
                                    self.config.theme == theme,
                                    format!("{:?}", theme),
                                )
                                .clicked()
                            {
                                self.config.theme = theme;
                                self.theme_manager.set_theme(theme);
                                self.theme_manager.apply_theme(ctx);
                            }
                        }
                    });

                    ui.separator();
                    ui.checkbox(&mut self.config.hot_reload_enabled, "Hot Reload");
                });

                // Tool-specific menu items
                if let Err(e) = self.tool.menu_bar(ui) {
                    warn!("Tool menu error: {}", e);
                }

                // Help menu
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        // Show about dialog
                    }
                });

                // Performance info (right-aligned)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let fps = self.performance_monitor.get_fps();
                    ui.label(format!("FPS: {:.0}", fps));
                });
            });
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Ready");

                // Right-aligned status info
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mem_usage = self.performance_monitor.get_memory_usage();
                    ui.label(format!("Memory: {:.1} MB", mem_usage));
                });
            });
        });

        // Main tool content
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
