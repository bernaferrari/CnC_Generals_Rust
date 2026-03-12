//! Core World Builder editor implementation

use crate::map::{Map, MapSettings};
use crate::objects::ObjectManager;
use crate::scripting::ScriptEditor;
use crate::terrain::TerrainEditor;
use crate::tools::ToolManager;
use crate::ui::WorldBuilderUI;

use anyhow::Result;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use ui_framework::{
    dialogs::{DialogManager, FileDialog, FileDialogAction, FileDialogType},
    GameTool, ThemeType, ToolConfig, Viewport3D,
};
use uuid::Uuid;

/// Main World Builder tool implementation
pub struct WorldBuilderTool {
    id: Uuid,
    config: ToolConfig,

    // Core components
    current_map: Option<Arc<RwLock<Map>>>,
    terrain_editor: TerrainEditor,
    object_manager: ObjectManager,
    script_editor: ScriptEditor,
    tool_manager: ToolManager,

    // UI components
    ui: WorldBuilderUI,
    viewport: Viewport3D,
    dialog_manager: DialogManager,

    // State
    is_initialized: bool,
    dirty: bool, // Has unsaved changes
    last_save_path: Option<PathBuf>,

    // Performance
    frame_count: u64,
    last_fps_update: std::time::Instant,
    current_fps: f64,
}

impl WorldBuilderTool {
    pub fn new() -> Result<Self> {
        let id = Uuid::new_v4();
        let mut config = ToolConfig::default();
        config.name = "World Builder".to_string();
        config.version = env!("CARGO_PKG_VERSION").to_string();
        config.window_size = [1400.0, 900.0];
        config.theme = ThemeType::Modern;

        Ok(Self {
            id,
            config,

            current_map: None,
            terrain_editor: TerrainEditor::new(),
            object_manager: ObjectManager::new(),
            script_editor: ScriptEditor::new(),
            tool_manager: ToolManager::new(),

            ui: WorldBuilderUI::new(),
            viewport: Viewport3D::new(),
            dialog_manager: DialogManager::new(),

            is_initialized: false,
            dirty: false,
            last_save_path: None,

            frame_count: 0,
            last_fps_update: std::time::Instant::now(),
            current_fps: 0.0,
        })
    }

    /// Create a new map
    pub fn new_map(&mut self, settings: MapSettings) -> Result<()> {
        let map = Map::new(settings)?;
        let map_arc = Arc::new(RwLock::new(map));
        self.current_map = Some(map_arc.clone());
        self.dirty = true;
        self.last_save_path = None;

        // Initialize terrain and objects for new map
        self.terrain_editor.set_map(Some(map_arc.clone()))?;
        self.object_manager.clear();
        self.script_editor.clear();

        let map_guard = map_arc.read().unwrap();
        log::info!(
            "Created new map: {}x{}",
            map_guard.width(),
            map_guard.height()
        );

        Ok(())
    }

    /// Load a map from file
    pub fn load_map(&mut self, path: PathBuf) -> Result<()> {
        log::info!("Loading map from: {}", path.display());

        // Run async operation in blocking context
        let map = tokio::runtime::Runtime::new()?.block_on(Map::load(&path))?;

        let map_arc = Arc::new(RwLock::new(map));
        self.current_map = Some(map_arc.clone());
        self.last_save_path = Some(path.clone());
        self.dirty = false;

        // Update editors with loaded map
        self.terrain_editor.set_map(Some(map_arc.clone()))?;

        let map_guard = map_arc.read().unwrap();
        self.object_manager.load_objects(&map_guard)?;
        self.script_editor.load_scripts(&map_guard)?;
        drop(map_guard);

        log::info!("Successfully loaded map: {}", path.display());
        Ok(())
    }

    /// Save the current map
    pub fn save_map(&mut self) -> Result<()> {
        if let Some(ref path) = self.last_save_path.clone() {
            self.save_map_as(path.clone())
        } else {
            // Open save dialog
            self.dialog_manager.open_dialog(
                "save_map".to_string(),
                Box::new(FileDialog::new(FileDialogType::Save, "map")),
            );
            Ok(())
        }
    }

    /// Save the map to a specific path
    pub fn save_map_as(&mut self, path: PathBuf) -> Result<()> {
        if let Some(ref map_arc) = self.current_map {
            // Update map with current editor state
            let mut map_guard = map_arc.write().unwrap();
            self.terrain_editor.save_to_map(&mut *map_guard)?;
            self.object_manager.save_to_map(&mut *map_guard)?;
            self.script_editor.save_to_map(&mut *map_guard)?;

            // Save to file (run async in blocking context)
            tokio::runtime::Runtime::new()?.block_on(map_guard.save(&path))?;

            drop(map_guard);

            self.last_save_path = Some(path.clone());
            self.dirty = false;

            log::info!("Saved map to: {}", path.display());
        }

        Ok(())
    }

    /// Check if there are unsaved changes
    pub fn has_unsaved_changes(&self) -> bool {
        self.dirty
            || self.terrain_editor.has_unsaved_changes()
            || self.object_manager.has_unsaved_changes()
            || self.script_editor.has_unsaved_changes()
    }

    /// Get the current map name
    pub fn current_map_name(&self) -> String {
        if let Some(ref map_arc) = self.current_map {
            let map = map_arc.read().unwrap();
            map.name().to_string()
        } else {
            "No Map Loaded".to_string()
        }
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

    /// Handle viewport input and updates
    fn update_viewport(&mut self, ui: &mut egui::Ui) -> Result<()> {
        // Update 3D viewport
        self.viewport.update(ui)?;

        // Handle tool-specific viewport interaction
        if let Some(active_tool) = self.tool_manager.active_tool_mut() {
            active_tool.handle_viewport_input(&mut self.viewport, ui)?;
        }

        Ok(())
    }

    /// Process pending file dialogs
    fn process_dialogs(&mut self) -> Result<()> {
        // Check for file dialog results
        if let Some(dialog) = self.dialog_manager.get_panel_mut("save_map") {
            if let Some(file_dialog) = dialog.as_any_mut().downcast_mut::<FileDialog>() {
                if let Some(result) = file_dialog.get_result() {
                    match result.action {
                        FileDialogAction::Save => {
                            self.save_map_as(PathBuf::from(result.path))?;
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }
}

impl GameTool for WorldBuilderTool {
    fn id(&self) -> Uuid {
        self.id
    }

    fn name(&self) -> &str {
        "World Builder"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn initialize(&mut self) -> Result<()> {
        if self.is_initialized {
            return Ok(());
        }

        log::info!("Initializing World Builder...");

        // Initialize components
        self.terrain_editor.initialize()?;
        self.object_manager.initialize()?;
        self.script_editor.initialize()?;
        self.tool_manager.initialize()?;
        self.ui.initialize()?;

        // Set up initial camera position
        self.viewport.set_camera(
            glam::Vec3::new(0.0, 50.0, 100.0), // position
            glam::Vec3::ZERO,                  // target
        );

        self.is_initialized = true;
        log::info!("World Builder initialized successfully");

        Ok(())
    }

    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) -> Result<()> {
        self.update_fps();

        // Process dialogs first
        self.dialog_manager.update(ctx);
        self.process_dialogs()?;

        // Main editor layout
        egui::SidePanel::left("tool_panel")
            .resizable(true)
            .default_width(250.0)
            .show(ctx, |ui| {
                self.ui.show_tool_panel(
                    ui,
                    &mut self.tool_manager,
                    &mut self.terrain_editor,
                    &mut self.object_manager,
                );
            });

        egui::SidePanel::right("properties_panel")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                self.ui
                    .show_properties_panel(ui, &mut self.object_manager, &self.tool_manager);
            });

        egui::TopBottomPanel::bottom("status_bar")
            .resizable(false)
            .default_height(25.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Map info
                    ui.label(format!("Map: {}", self.current_map_name()));

                    ui.separator();

                    // Tool status
                    if let Some(tool) = self.tool_manager.active_tool() {
                        ui.label(format!("Tool: {}", tool.name()));
                    }

                    ui.separator();

                    // Dirty flag
                    if self.has_unsaved_changes() {
                        ui.colored_label(egui::Color32::YELLOW, "Unsaved Changes");
                    } else {
                        ui.label("Saved");
                    }

                    // Right-aligned status
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("FPS: {:.0}", self.current_fps));

                        if let Some(ref map_arc) = self.current_map {
                            let map = map_arc.read().unwrap();
                            ui.label(format!("Size: {}x{}", map.width(), map.height()));
                        }
                    });
                });
            });

        // Main viewport area
        let viewport_result = egui::CentralPanel::default().show(ctx, |ui| {
            // Viewport toolbar
            ui.horizontal(|ui| {
                self.ui
                    .show_viewport_toolbar(ui, &mut self.viewport, &mut self.tool_manager);
            });

            ui.separator();

            // 3D viewport
            self.update_viewport(ui)
        });

        // Handle viewport result
        viewport_result.inner?;

        // Update editors
        if let Some(ref map) = self.current_map {
            self.terrain_editor.update()?;
            self.object_manager.update()?;
            self.script_editor.update()?;
        }

        // Update tools
        self.tool_manager.update()?;

        Ok(())
    }

    fn menu_bar(&mut self, ui: &mut eframe::egui::Ui) -> Result<()> {
        // File menu (additional items)
        ui.menu_button("Map", |ui| {
            if ui.button("New Map...").clicked() {
                // Open new map dialog
                self.ui.show_new_map_dialog();
            }

            if ui.button("Load Map...").clicked() {
                // Open load dialog
                self.dialog_manager.open_dialog(
                    "load_map".to_string(),
                    Box::new(FileDialog::new(FileDialogType::Open, "map")),
                );
            }

            ui.separator();

            if ui.button("Save Map").clicked() {
                if let Err(e) = self.save_map() {
                    log::error!("Failed to save map: {}", e);
                }
            }

            if ui.button("Save Map As...").clicked() {
                self.dialog_manager.open_dialog(
                    "save_map_as".to_string(),
                    Box::new(FileDialog::new(FileDialogType::SaveAs, "map")),
                );
            }

            ui.separator();

            if ui.button("Export...").clicked() {
                // Export map for game engine
            }
        });

        // Tools menu
        ui.menu_button("Tools", |ui| {
            for tool_id in self.tool_manager.available_tools() {
                let tool_name = self
                    .tool_manager
                    .get_tool_name(&tool_id)
                    .unwrap_or(&tool_id);
                if ui.button(tool_name).clicked() {
                    self.tool_manager.set_active_tool(&tool_id);
                }
            }

            ui.separator();

            if ui.button("Validate Map").clicked() {
                // Run map validation
            }
        });

        // View menu
        ui.menu_button("Viewport", |ui| {
            if ui.button("Reset Camera").clicked() {
                self.viewport
                    .set_camera(glam::Vec3::new(0.0, 50.0, 100.0), glam::Vec3::ZERO);
            }

            ui.separator();

            if ui.button("Wireframe").clicked() {
                // Toggle wireframe mode
            }

            if ui.button("Show Grid").clicked() {
                // Toggle grid display
            }
        });

        Ok(())
    }

    fn shutdown(&mut self) -> Result<()> {
        log::info!("Shutting down World Builder...");

        // Check for unsaved changes
        if self.has_unsaved_changes() {
            log::warn!("World Builder shutting down with unsaved changes");
        }

        // Save configuration
        // TODO: Save window state, recent files, etc.

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

/// World Builder specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldBuilderConfig {
    pub grid_size: f32,
    pub snap_to_grid: bool,
    pub auto_save_enabled: bool,
    pub auto_save_interval: u32, // minutes
    pub recent_maps: Vec<PathBuf>,
    pub default_map_settings: MapSettings,
    pub viewport_settings: ViewportSettings,
}

impl Default for WorldBuilderConfig {
    fn default() -> Self {
        Self {
            grid_size: 1.0,
            snap_to_grid: true,
            auto_save_enabled: true,
            auto_save_interval: 5,
            recent_maps: Vec::new(),
            default_map_settings: MapSettings::default(),
            viewport_settings: ViewportSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportSettings {
    pub camera_speed: f32,
    pub mouse_sensitivity: f32,
    pub show_grid: bool,
    pub show_wireframe: bool,
    pub background_color: [f32; 3],
}

impl Default for ViewportSettings {
    fn default() -> Self {
        Self {
            camera_speed: 10.0,
            mouse_sensitivity: 0.005,
            show_grid: true,
            show_wireframe: false,
            background_color: [0.2, 0.2, 0.3],
        }
    }
}
