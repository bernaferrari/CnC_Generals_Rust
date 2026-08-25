//! Core Particle Editor implementation

use crate::chrome::{
    ChromeAction, ChromeViewState, SystemListCommand, show_center_panel, show_menu_bar,
    show_properties_panel, show_status_bar, show_system_list,
};
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

    // Core components — list + selection (C++ m_listOfParticleSystems)
    systems: Vec<ParticleSystem>,
    selected_index: Option<usize>,
    system_templates: HashMap<String, ParticleSystemTemplate>,

    // Editor components
    timeline: Timeline,
    preview: ParticlePreview,
    ui: ParticleEditorUI,
    chrome: ChromeViewState,
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

        let mut tool = Self {
            id,
            config,

            systems: Vec::new(),
            selected_index: None,
            system_templates: HashMap::new(),

            timeline: Timeline::new(),
            preview: ParticlePreview::new(),
            ui: ParticleEditorUI::new(),
            chrome: ChromeViewState::new(),
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
        };
        tool.load_templates()?;
        Ok(tool)
    }

    /// Create a new particle system and select it.
    pub fn new_system(&mut self, template: Option<&str>) -> Result<()> {
        let mut system = if let Some(template_name) = template {
            if let Some(template) = self.system_templates.get(template_name).cloned() {
                ParticleSystem::from_template(&template)?
            } else {
                ParticleSystem::new("New System".to_string())?
            }
        } else {
            ParticleSystem::new("New System".to_string())?
        };

        system.info.name = self.unique_name(&system.info.name);
        self.add_and_select(system);
        log::info!("Created new particle system");
        Ok(())
    }

    /// Create a named system (chrome / tests). Returns the new index.
    pub fn create_system(&mut self, name: &str) -> Result<usize> {
        let name = self.unique_name(name);
        let system = ParticleSystem::new(name)?;
        Ok(self.add_and_select(system))
    }

    /// Load a particle system from JSON or C++ INI and select it.
    pub fn load_system(&mut self, path: PathBuf) -> Result<()> {
        log::info!("Loading particle system from: {}", path.display());

        let system = match path.extension().and_then(|s| s.to_str()) {
            Some("ini") | Some("INI") => self.exporter.import_particle_system(&path)?,
            _ => ParticleSystem::load(&path)?,
        };
        self.add_and_select(system);

        self.last_save_path = Some(path.clone());
        self.dirty = false;

        log::info!("Successfully loaded particle system: {}", path.display());
        Ok(())
    }

    /// Save the current particle system
    pub fn save_system(&mut self) -> Result<()> {
        if let Some(ref path) = self.last_save_path.clone() {
            self.save_system_as(path.clone())
        } else {
            self.pick_save_path()
        }
    }

    /// Save the particle system to a specific path
    pub fn save_system_as(&mut self, path: PathBuf) -> Result<()> {
        if let Some(system) = self.selected_system() {
            if path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("ini"))
            {
                self.exporter.export_for_game_engine(system, &path)?;
            } else {
                system.save(&path)?;
            }

            self.last_save_path = Some(path.clone());
            self.dirty = false;

            log::info!("Saved particle system to: {}", path.display());
        }

        Ok(())
    }

    /// Export the selected particle system as C++ `_writeSingleParticleSystem` INI.
    pub fn export_system(&mut self, path: PathBuf) -> Result<()> {
        if let Some(system) = self.selected_system() {
            self.exporter.export_for_game_engine(system, &path)?;
            log::info!("Exported particle system to: {}", path.display());
        }
        Ok(())
    }

    /// Shipped INI text for the selected system (chrome / tests).
    pub fn export_selected_ini(&self) -> Option<String> {
        self.selected_system()
            .map(|system| self.exporter.generate_ini_content(system))
    }

    pub fn system_count(&self) -> usize {
        self.systems.len()
    }

    pub fn systems(&self) -> &[ParticleSystem] {
        &self.systems
    }

    pub fn system_names(&self) -> Vec<String> {
        self.systems.iter().map(|s| s.info.name.clone()).collect()
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    pub fn selected_system(&self) -> Option<&ParticleSystem> {
        self.selected_index.and_then(|i| self.systems.get(i))
    }

    pub fn selected_system_mut(&mut self) -> Option<&mut ParticleSystem> {
        self.selected_index.and_then(|i| self.systems.get_mut(i))
    }

    pub fn selected_name(&self) -> Option<String> {
        self.selected_system().map(|s| s.info.name.clone())
    }

    pub fn chrome(&self) -> &ChromeViewState {
        &self.chrome
    }

    pub fn chrome_mut(&mut self) -> &mut ChromeViewState {
        &mut self.chrome
    }

    /// Select a system by index (left-list click).
    pub fn select_system(&mut self, index: usize) -> Result<()> {
        if index >= self.systems.len() {
            anyhow::bail!("invalid particle system index {index}");
        }
        self.selected_index = Some(index);
        let system = self.systems.get(index);
        self.timeline.set_system(system)?;
        self.preview.set_system(system)?;
        self.current_time = 0.0;
        self.is_playing = false;
        Ok(())
    }

    /// Dispatch a chrome File/Edit action.
    pub fn apply_chrome_action(&mut self, action: ChromeAction) -> Result<()> {
        match action {
            ChromeAction::NewSystem => self.new_system(None),
            ChromeAction::Open => self.pick_open_path(),
            ChromeAction::Save => self.save_system(),
            ChromeAction::ExportIni => self.pick_export_path(),
            ChromeAction::Exit => {
                self.chrome.exit_requested = true;
                Ok(())
            }
            ChromeAction::ResetSystem => {
                if let Some(system) = self.selected_system_mut() {
                    system.reset();
                }
                self.current_time = 0.0;
                Ok(())
            }
            ChromeAction::DuplicateSystem => self.duplicate_selected(),
            ChromeAction::DeleteSystem => self.delete_selected(),
        }
    }

    fn add_and_select(&mut self, system: ParticleSystem) -> usize {
        let idx = self.systems.len();
        self.systems.push(system);
        if let Err(e) = self.select_system(idx) {
            log::error!("Failed to select new system: {e}");
        }
        self.dirty = true;
        idx
    }

    fn unique_name(&self, base: &str) -> String {
        if !self.systems.iter().any(|s| s.info.name == base) {
            return base.to_string();
        }
        let mut i = 2;
        loop {
            let candidate = format!("{base} {i}");
            if !self.systems.iter().any(|s| s.info.name == candidate) {
                return candidate;
            }
            i += 1;
        }
    }

    fn duplicate_selected(&mut self) -> Result<()> {
        let Some(mut copy) = self.selected_system().cloned() else {
            return Ok(());
        };
        copy.info.name = self.unique_name(&format!("{} Copy", copy.info.name));
        copy.reset();
        self.add_and_select(copy);
        Ok(())
    }

    fn delete_selected(&mut self) -> Result<()> {
        let Some(i) = self.selected_index else {
            return Ok(());
        };
        if i >= self.systems.len() {
            return Ok(());
        }
        self.systems.remove(i);
        self.dirty = true;
        if self.systems.is_empty() {
            self.selected_index = None;
            self.timeline.set_system(None)?;
            self.preview.set_system(None)?;
        } else {
            self.select_system(i.min(self.systems.len() - 1))?;
        }
        Ok(())
    }

    fn pick_open_path(&mut self) -> Result<()> {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Particle", &["ini", "json"])
            .pick_file()
        {
            self.load_system(path)?;
        }
        Ok(())
    }

    fn pick_save_path(&mut self) -> Result<()> {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Particle JSON", &["json"])
            .add_filter("Particle INI", &["ini"])
            .save_file()
        {
            self.save_system_as(path)?;
        } else {
            log::warn!("No save path set, please use Save As");
        }
        Ok(())
    }

    fn pick_export_path(&mut self) -> Result<()> {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Particle INI", &["ini"])
            .save_file()
        {
            self.export_system(path)?;
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

        if let Some(system) = self.selected_system_mut() {
            system.reset();
        }

        log::info!("Stopped particle system playback");
    }

    /// Seek to specific time
    pub fn seek_to(&mut self, time: f32) {
        self.current_time = time.max(0.0);

        let time = self.current_time;
        if let Some(system) = self.selected_system_mut() {
            system.seek_to(time);
        }

        self.timeline.set_current_time(self.current_time);
    }

    /// Get the current system name
    pub fn current_system_name(&self) -> String {
        self.selected_name()
            .unwrap_or_else(|| "No System Loaded".to_string())
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
            let step = dt * self.playback_speed;
            let time = self.current_time + step;
            self.current_time = time;
            let duration = self.timeline.duration();
            let looping = self.timeline.is_looping();
            let reached_end = time >= duration;

            if let Some(system) = self.selected_system_mut() {
                system.update(step)?;
                if reached_end && looping {
                    system.reset();
                }
            }
            if reached_end {
                if looping {
                    self.current_time = 0.0;
                } else {
                    self.is_playing = false;
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

    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) -> Result<()> {
        self.update_fps();

        // Calculate delta time
        let dt = ctx.input(|i| i.unstable_dt).min(1.0 / 30.0); // Cap at 30fps minimum

        // Update simulation
        self.update_simulation(dt)?;

        if self.chrome.exit_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // Status first so it sits on the outer bottom edge.
        let status_name = self.current_system_name();
        let system_count = self.systems.len();
        let dirty = self.dirty;
        let fps = self.current_fps;
        egui::TopBottomPanel::bottom("particle_editor_status")
            .resizable(false)
            .default_height(25.0)
            .show(ctx, |ui| {
                show_status_bar(ui, system_count, &status_name, dirty, fps);
            });

        if self.chrome.show_timeline {
            egui::TopBottomPanel::bottom("timeline_panel")
                .resizable(true)
                .default_height(200.0)
                .show(ctx, |ui| {
                    self.timeline
                        .show(ui, &mut self.current_time, &mut self.is_playing);

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

                        if let Some(system) = self.selected_system() {
                            ui.label(format!("Particles: {}", system.particle_count()));
                        }
                    });
                });
        }

        let mut list_cmd = SystemListCommand::None;
        egui::SidePanel::left("systems_list")
            .resizable(true)
            .default_width(220.0)
            .show(ctx, |ui| {
                list_cmd = show_system_list(
                    ui,
                    &self.systems,
                    self.selected_index,
                    &self.system_templates,
                );
            });

        match list_cmd {
            SystemListCommand::None => {}
            SystemListCommand::Select(i) => {
                if let Err(e) = self.select_system(i) {
                    log::error!("Failed to select system: {e}");
                }
            }
            SystemListCommand::NewBlank => {
                if let Err(e) = self.new_system(None) {
                    log::error!("Failed to create system: {e}");
                }
            }
            SystemListCommand::NewFromTemplate(name) => {
                if let Err(e) = self.new_system(Some(&name)) {
                    log::error!("Failed to create system from template: {e}");
                }
            }
        }

        let mut properties_changed = false;
        if self.chrome.show_properties {
            egui::SidePanel::right("properties_panel")
                .resizable(true)
                .default_width(320.0)
                .show(ctx, |ui| {
                    let selected = self.selected_index.and_then(|i| self.systems.get_mut(i));
                    properties_changed = show_properties_panel(ui, selected);
                });
        }
        if properties_changed {
            self.dirty = true;
        }

        let show_preview = self.chrome.show_preview;
        let current_time = self.current_time;
        let mut center_changed = false;
        egui::CentralPanel::default().show(ctx, |ui| {
            let selected = self.selected_index.and_then(|i| self.systems.get_mut(i));
            center_changed =
                show_center_panel(ui, selected, &mut self.preview, current_time, show_preview);
        });
        if center_changed {
            self.dirty = true;
        }

        self.timeline.update(dt)?;
        self.preview.update(dt)?;

        Ok(())
    }

    fn menu_bar(&mut self, ui: &mut eframe::egui::Ui) -> Result<()> {
        if let Some(action) = show_menu_bar(ui, &mut self.chrome) {
            if let Err(e) = self.apply_chrome_action(action) {
                log::error!("Chrome menu action failed: {e}");
            }
        }

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
