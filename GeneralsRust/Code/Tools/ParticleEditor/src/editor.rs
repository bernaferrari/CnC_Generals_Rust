//! Core Particle Editor implementation

use crate::export::ParticleExporter;
use crate::particles::{ParticleSystem, ParticleSystemTemplate};
use crate::preview::ParticlePreview;
use crate::timeline::Timeline;
use crate::ui::ParticleEditorUI;

use anyhow::Result;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use ui_framework::{GameTool, ThemeType, ToolConfig};
use uuid::Uuid;

/// Main Particle Editor tool implementation
pub struct ParticleEditorTool {
    id: Uuid,
    config: ToolConfig,

    // Core components
    current_system: Option<ParticleSystem>,
    system_templates: HashMap<String, ParticleSystemTemplate>,

    // Editor components
    timeline: Timeline,
    preview: ParticlePreview,
    ui: ParticleEditorUI,
    exporter: ParticleExporter,

    // State
    is_initialized: bool,
    is_playing: bool,
    current_time: f32,
    playback_speed: f32,
    dirty: bool,
    last_save_path: Option<PathBuf>,

    // Performance
    frame_count: u64,
    last_fps_update: std::time::Instant,
    current_fps: f64,
}

impl ParticleEditorTool {
    pub fn new() -> Result<Self> {
        let id = Uuid::new_v4();
        let mut config = ToolConfig::default();
        config.name = "Particle Editor".to_string();
        config.version = env!("CARGO_PKG_VERSION").to_string();
        config.window_size = [1200.0, 800.0];
        config.theme = ThemeType::Dark;

        Ok(Self {
            id,
            config,

            current_system: None,
            system_templates: HashMap::new(),

            timeline: Timeline::new(),
            preview: ParticlePreview::new(),
            ui: ParticleEditorUI::new(),
            exporter: ParticleExporter::new(),

            is_initialized: false,
            is_playing: false,
            current_time: 0.0,
            playback_speed: 1.0,
            dirty: false,
            last_save_path: None,

            frame_count: 0,
            last_fps_update: std::time::Instant::now(),
            current_fps: 0.0,
        })
    }

    /// Create a new particle system
    pub fn new_system(&mut self, template: Option<&str>) -> Result<()> {
        let system = if let Some(template_name) = template {
            if let Some(template) = self.system_templates.get(template_name) {
                ParticleSystem::from_template(template)?
            } else {
                ParticleSystem::new("New System".to_string())?
            }
        } else {
            ParticleSystem::new("New System".to_string())?
        };

        self.current_system = Some(system);
        self.timeline.set_system(self.current_system.as_ref())?;
        self.preview.set_system(self.current_system.as_ref())?;

        self.current_time = 0.0;
        self.is_playing = false;
        self.dirty = true;
        self.last_save_path = None;

        log::info!("Created new particle system");
        Ok(())
    }

    /// Load a particle system from file
    pub fn load_system(&mut self, path: PathBuf) -> Result<()> {
        log::info!("Loading particle system from: {}", path.display());

        let system = ParticleSystem::load(&path)?;
        self.current_system = Some(system);

        self.timeline.set_system(self.current_system.as_ref())?;
        self.preview.set_system(self.current_system.as_ref())?;

        self.last_save_path = Some(path.clone());
        self.current_time = 0.0;
        self.is_playing = false;
        self.dirty = false;

        log::info!("Successfully loaded particle system: {}", path.display());
        Ok(())
    }

    /// Save the current particle system
    pub fn save_system(&mut self) -> Result<()> {
        if let Some(ref path) = self.last_save_path.clone() {
            self.save_system_as(path.clone())
        } else {
            // TODO: Implement file dialog for save
            log::warn!("No save path set, please use Save As");
            Ok(())
        }
    }

    /// Save the particle system to a specific path
    pub fn save_system_as(&mut self, path: PathBuf) -> Result<()> {
        if let Some(ref system) = self.current_system {
            system.save(&path)?;

            self.last_save_path = Some(path.clone());
            self.dirty = false;

            log::info!("Saved particle system to: {}", path.display());
        }

        Ok(())
    }

    /// Export the particle system for the game engine
    pub fn export_system(&mut self, path: PathBuf) -> Result<()> {
        if let Some(ref system) = self.current_system {
            self.exporter.export_for_game_engine(system, &path)?;
            log::info!("Exported particle system to: {}", path.display());
        }
        Ok(())
    }

    /// Play/pause the particle system preview
    pub fn toggle_playback(&mut self) {
        self.is_playing = !self.is_playing;

        if self.is_playing {
            log::info!("Started particle system playback");
        } else {
            log::info!("Paused particle system playback");
        }
    }

    /// Stop playback and reset to beginning
    pub fn stop_playback(&mut self) {
        self.is_playing = false;
        self.current_time = 0.0;

        if let Some(ref mut system) = self.current_system {
            system.reset();
        }

        log::info!("Stopped particle system playback");
    }

    /// Seek to specific time
    pub fn seek_to(&mut self, time: f32) {
        self.current_time = time.max(0.0);

        if let Some(ref mut system) = self.current_system {
            system.seek_to(self.current_time);
        }

        self.timeline.set_current_time(self.current_time);
    }

    /// Get the current system name
    pub fn current_system_name(&self) -> String {
        if let Some(ref system) = self.current_system {
            system.name().to_string()
        } else {
            "No System Loaded".to_string()
        }
    }

    /// Check if there are unsaved changes
    pub fn has_unsaved_changes(&self) -> bool {
        self.dirty
    }

    /// Update FPS calculation
    fn update_fps(&mut self) {
        self.frame_count += 1;

        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_fps_update);

        if elapsed.as_secs_f64() >= 1.0 {
            self.current_fps = self.frame_count as f64 / elapsed.as_secs_f64();
            self.frame_count = 0;
            self.last_fps_update = now;
        }
    }

    /// Update particle system simulation
    fn update_simulation(&mut self, dt: f32) -> Result<()> {
        if self.is_playing {
            self.current_time += dt * self.playback_speed;

            if let Some(ref mut system) = self.current_system {
                system.update(dt * self.playback_speed)?;

                // Check if we've reached the end of the timeline
                if self.current_time >= self.timeline.duration() {
                    if self.timeline.is_looping() {
                        self.current_time = 0.0;
                        system.reset();
                    } else {
                        self.is_playing = false;
                    }
                }
            }

            self.timeline.set_current_time(self.current_time);
        }

        Ok(())
    }

    /// Load particle system templates
    fn load_templates(&mut self) -> Result<()> {
        // Load built-in templates
        let templates = vec![
            ("Fire", ParticleSystemTemplate::fire()),
            ("Smoke", ParticleSystemTemplate::smoke()),
            ("Explosion", ParticleSystemTemplate::explosion()),
            ("Sparks", ParticleSystemTemplate::sparks()),
            ("Magic", ParticleSystemTemplate::magic()),
            ("Water", ParticleSystemTemplate::water()),
        ];

        for (name, template) in templates {
            self.system_templates.insert(name.to_string(), template);
        }

        Ok(())
    }
}

impl GameTool for ParticleEditorTool {
    fn id(&self) -> Uuid {
        self.id
    }

    fn name(&self) -> &str {
        "Particle Editor"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn initialize(&mut self) -> Result<()> {
        if self.is_initialized {
            return Ok(());
        }

        log::info!("Initializing Particle Editor...");

        // Initialize components
        self.timeline.initialize()?;
        self.preview.initialize()?;
        self.ui.initialize()?;
        self.exporter.initialize()?;

        // Load templates
        self.load_templates()?;

        self.is_initialized = true;
        log::info!("Particle Editor initialized successfully");

        Ok(())
    }

    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) -> Result<()> {
        self.update_fps();

        // Calculate delta time
        let dt = ctx.input(|i| i.unstable_dt).min(1.0 / 30.0); // Cap at 30fps minimum

        // Update simulation
        self.update_simulation(dt)?;

        // Main editor layout
        egui::SidePanel::left("properties_panel")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                self.ui.show_properties_panel(ui, &mut self.current_system);
            });

        // Handle template selection separately to avoid borrow checker issues
        let mut selected_template: Option<String> = None;

        egui::SidePanel::right("templates_panel")
            .resizable(true)
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.group(|ui| {
                    ui.heading("Templates");
                    ui.label("Select a template to create a new particle system:");
                    ui.separator();

                    for (name, _template) in &self.system_templates {
                        if ui.button(name).clicked() {
                            selected_template = Some(name.clone());
                        }
                    }
                });
            });

        // Apply template selection after the closure
        if let Some(template) = selected_template {
            if let Err(e) = self.new_system(Some(&template)) {
                log::error!("Failed to create system from template: {}", e);
            }
        }

        egui::TopBottomPanel::bottom("timeline_panel")
            .resizable(true)
            .default_height(200.0)
            .show(ctx, |ui| {
                self.timeline
                    .show(ui, &mut self.current_time, &mut self.is_playing);

                // Playback controls
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(if self.is_playing { "⏸" } else { "▶" }).clicked() {
                        self.toggle_playback();
                    }

                    if ui.button("⏹").clicked() {
                        self.stop_playback();
                    }

                    ui.separator();

                    ui.label("Speed:");
                    ui.add(egui::Slider::new(&mut self.playback_speed, 0.1..=5.0));

                    ui.separator();

                    ui.label(format!("Time: {:.2}s", self.current_time));

                    if self.current_system.is_some() {
                        ui.label(format!(
                            "Particles: {}",
                            self.current_system.as_ref().unwrap().particle_count()
                        ));
                    }
                });
            });

        egui::TopBottomPanel::bottom("status_bar")
            .resizable(false)
            .default_height(25.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("System: {}", self.current_system_name()));

                    ui.separator();

                    if self.has_unsaved_changes() {
                        ui.colored_label(egui::Color32::YELLOW, "Unsaved Changes");
                    } else {
                        ui.label("Saved");
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("FPS: {:.0}", self.current_fps));
                    });
                });
            });

        // Main preview area
        egui::CentralPanel::default().show(ctx, |ui| {
            self.preview
                .show(ui, self.current_system.as_ref(), self.current_time);
        });

        // Update components
        self.timeline.update(dt)?;
        self.preview.update(dt)?;

        Ok(())
    }

    fn menu_bar(&mut self, ui: &mut eframe::egui::Ui) -> Result<()> {
        // File menu (additional items)
        ui.menu_button("System", |ui| {
            if ui.button("New System").clicked() {
                if let Err(e) = self.new_system(None) {
                    log::error!("Failed to create new system: {}", e);
                }
            }

            if ui.button("Load System...").clicked() {
                // TODO: Implement file dialog for load
                log::info!("Load system requested");
            }

            ui.separator();

            if ui.button("Save System").clicked() {
                if let Err(e) = self.save_system() {
                    log::error!("Failed to save system: {}", e);
                }
            }

            if ui.button("Save System As...").clicked() {
                // TODO: Implement file dialog for save as
                log::info!("Save system as requested");
            }

            ui.separator();

            if ui.button("Export for Game...").clicked() {
                // TODO: Implement file dialog for export
                log::info!("Export system requested");
            }
        });

        // Edit menu
        ui.menu_button("Edit", |ui| {
            if ui.button("Reset System").clicked() {
                if let Some(ref mut system) = self.current_system {
                    system.reset();
                    self.current_time = 0.0;
                }
            }

            ui.separator();

            if ui.button("Duplicate System").clicked() {
                // Duplicate current system
            }
        });

        // Playback menu
        ui.menu_button("Playback", |ui| {
            if ui
                .button(if self.is_playing { "Pause" } else { "Play" })
                .clicked()
            {
                self.toggle_playback();
            }

            if ui.button("Stop").clicked() {
                self.stop_playback();
            }

            ui.separator();

            let mut is_looping = self.timeline.is_looping();
            if ui.checkbox(&mut is_looping, "Loop").changed() {
                self.timeline.is_looping = is_looping;
            }
        });

        Ok(())
    }

    fn shutdown(&mut self) -> Result<()> {
        log::info!("Shutting down Particle Editor...");

        if self.has_unsaved_changes() {
            log::warn!("Particle Editor shutting down with unsaved changes");
        }

        Ok(())
    }

    fn config(&self) -> &ToolConfig {
        &self.config
    }

    fn set_config(&mut self, config: ToolConfig) -> Result<()> {
        self.config = config;
        Ok(())
    }
}

/// Particle Editor specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticleEditorConfig {
    pub auto_play_on_load: bool,
    pub default_playback_speed: f32,
    pub max_particles_per_system: u32,
    pub preview_quality: PreviewQuality,
    pub timeline_snap_enabled: bool,
    pub timeline_snap_interval: f32,
}

impl Default for ParticleEditorConfig {
    fn default() -> Self {
        Self {
            auto_play_on_load: true,
            default_playback_speed: 1.0,
            max_particles_per_system: 10000,
            preview_quality: PreviewQuality::High,
            timeline_snap_enabled: true,
            timeline_snap_interval: 0.1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreviewQuality {
    Low,
    Medium,
    High,
    Ultra,
}
