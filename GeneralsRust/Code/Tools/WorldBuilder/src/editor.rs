//! Core World Builder editor implementation

use crate::map::{Map, MapSettings};
use crate::objects::ObjectManager;
use crate::scripting::ScriptEditor;
use crate::terrain::TerrainEditor;
use crate::tools::ToolManager;
use crate::ui::{
    ChromeCommand, EditorChrome, WbToolId, WorldBuilderUI, apply_chrome_view_command, world_to_cell,
};

use anyhow::Result;
use eframe::egui;
use game_engine::map_object::{Coord3D, MAP_XY_FACTOR, MapObject};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use ui_framework::{
    GameTool, ThemeType, ToolConfig, Viewport3D,
    dialogs::{DialogManager, FileDialog, FileDialogAction, FileDialogType},
};
use uuid::Uuid;
use world_builder::scorch_tool::{DEFAULT_SCORCHMARK_RADIUS, SCORCH_1, mouse_down_scorch};

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
    chrome: EditorChrome,

    // State
    is_initialized: bool,
    dirty: bool, // Has unsaved changes
    last_save_path: Option<PathBuf>,
    /// Scorch map objects for C++ `ScorchTool::mouseDown` / `pickScorch`.
    scorch_objects: Vec<MapObject>,

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
            chrome: EditorChrome::new(),

            is_initialized: false,
            dirty: false,
            last_save_path: None,
            scorch_objects: Vec::new(),

            frame_count: 0,
            last_fps_update: std::time::Instant::now(),
            current_fps: 0.0,
        })
    }

    /// Switch the live editor tool (palette / Tools menu / tests).
    pub fn set_current_tool(&mut self, tool_id: &str) -> bool {
        if !self.chrome.select_tool_id(tool_id) {
            return false;
        }
        let _ = self.chrome.take_command();
        if !self.tool_manager.has_tool(tool_id) {
            return false;
        }
        self.tool_manager.set_active_tool(tool_id);
        true
    }

    pub fn current_tool_id(&self) -> &str {
        self.chrome.selected_tool_id()
    }

    pub fn current_tool_name(&self) -> &str {
        self.chrome.selected_tool_name()
    }

    pub fn chrome(&self) -> &EditorChrome {
        &self.chrome
    }

    pub fn scorch_objects(&self) -> &[MapObject] {
        &self.scorch_objects
    }

    /// C++ `ScorchTool::mouseDown` when the scorch tool is current.
    pub fn scorch_mouse_down(&mut self, loc: Coord3D) -> Option<usize> {
        if self.chrome.selected_tool() != WbToolId::Scorch {
            return None;
        }
        Some(mouse_down_scorch(
            &mut self.scorch_objects,
            loc,
            DEFAULT_SCORCHMARK_RADIUS,
            SCORCH_1,
        ))
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
        self.chrome.set_map_name(map_guard.name());
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
        self.chrome.set_map_name(map_guard.name());
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

        self.update_hover_and_scorch(ui);

        // Handle tool-specific viewport interaction
        if let Some(active_tool) = self.tool_manager.active_tool_mut() {
            active_tool.handle_viewport_input(&mut self.viewport, ui)?;
        }

        Ok(())
    }

    fn update_hover_and_scorch(&mut self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        let hover = ui.input(|i| i.pointer.hover_pos());
        let clicked = ui.input(|i| i.pointer.primary_pressed());
        if let Some(pos) = hover {
            if rect.contains(pos) {
                let world_x = pos.x - rect.min.x;
                let world_y = pos.y - rect.min.y;
                let world_z = 0.0;
                let cell = world_to_cell(world_x, world_y, MAP_XY_FACTOR);
                self.chrome
                    .set_hover_coords(Some(cell), Some((world_x, world_y, world_z)));
                if clicked {
                    self.scorch_mouse_down(Coord3D::new(world_x, world_y, world_z));
                }
            }
        }
    }

    fn apply_chrome_command(&mut self, ctx: &egui::Context, command: ChromeCommand) -> Result<()> {
        match command {
            ChromeCommand::FileNew => {
                self.ui.show_new_map_dialog();
            }
            ChromeCommand::FileOpen => {
                self.dialog_manager.open_dialog(
                    "load_map".to_string(),
                    Box::new(FileDialog::new(FileDialogType::Open, "map")),
                );
            }
            ChromeCommand::FileSave => {
                if let Err(e) = self.save_map() {
                    log::error!("Failed to save map: {}", e);
                }
            }
            ChromeCommand::FileSaveAs => {
                self.dialog_manager.open_dialog(
                    "save_map_as".to_string(),
                    Box::new(FileDialog::new(FileDialogType::SaveAs, "map")),
                );
            }
            ChromeCommand::FileExit => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ChromeCommand::EditUndo => {
                if let Err(e) = self.terrain_editor.undo() {
                    log::error!("Undo failed: {}", e);
                }
            }
            ChromeCommand::EditRedo => {
                if let Err(e) = self.terrain_editor.redo() {
                    log::error!("Redo failed: {}", e);
                }
            }
            ChromeCommand::SelectTool(tool) => {
                let _ = self.set_current_tool(tool.as_str());
            }
            ChromeCommand::ViewToggle(_) | ChromeCommand::HelpAbout => {
                apply_chrome_view_command(&mut self.chrome, &command);
            }
        }
        Ok(())
    }

    fn drain_chrome_commands(&mut self, ctx: &egui::Context) -> Result<()> {
        while let Some(command) = self.chrome.take_command() {
            self.apply_chrome_command(ctx, command)?;
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

        self.chrome.set_map_name(self.current_map_name());
        self.chrome.set_unsaved(self.has_unsaved_changes());

        if let Some(settings) = self.ui.process_new_map_dialog(ctx) {
            if let Err(e) = self.new_map(settings) {
                log::error!("Failed to create map: {}", e);
            }
        }
        self.ui.show_about_dialog(ctx, &mut self.chrome);

        // C++-matching File/Edit/View/Tools/Help (in addition to ToolApp chrome).
        egui::TopBottomPanel::top("wb_main_menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                self.ui.show_main_menu(ui, &mut self.chrome);
            });
        });
        self.drain_chrome_commands(ctx)?;

        // Main editor layout
        egui::SidePanel::left("tool_panel")
            .resizable(true)
            .default_width(250.0)
            .show(ctx, |ui| {
                self.ui.show_tool_panel(
                    ui,
                    &mut self.chrome,
                    &mut self.tool_manager,
                    &mut self.terrain_editor,
                    &mut self.object_manager,
                );
            });
        self.drain_chrome_commands(ctx)?;

        egui::SidePanel::right("properties_panel")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                self.ui
                    .show_properties_panel(ui, &mut self.object_manager, &self.tool_manager);
            });

        if self.chrome.show_status_bar {
            egui::TopBottomPanel::bottom("wb_status_bar")
                .resizable(false)
                .default_height(25.0)
                .show(ctx, |ui| {
                    let map_size = self.current_map.as_ref().map(|map_arc| {
                        let map = map_arc.read().unwrap();
                        (map.width(), map.height())
                    });
                    self.ui
                        .show_status_bar(ui, &self.chrome, self.current_fps, map_size);
                });
        }

        // Main viewport area
        let viewport_result = egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                self.ui.show_viewport_toolbar(
                    ui,
                    &mut self.viewport,
                    &mut self.tool_manager,
                    &mut self.chrome,
                );
            });

            ui.separator();

            self.update_viewport(ui)
        });

        viewport_result.inner?;

        if let Some(ref _map) = self.current_map {
            self.terrain_editor.update()?;
            self.object_manager.update()?;
            self.script_editor.update()?;
        }

        self.tool_manager.update()?;

        Ok(())
    }

    fn menu_bar(&mut self, ui: &mut eframe::egui::Ui) -> Result<()> {
        self.ui.show_main_menu(ui, &mut self.chrome);
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

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::map_object::Coord3D;

    #[test]
    fn chrome_menu_contains_file_open() {
        let tool = WorldBuilderTool::new().expect("WorldBuilderTool::new");
        assert!(
            tool.chrome().menu_contains("File", "Open"),
            "editor chrome must expose File/Open"
        );
    }

    #[test]
    fn selecting_scorch_tool_sets_current_tool() {
        let mut tool = WorldBuilderTool::new().expect("WorldBuilderTool::new");
        assert_eq!(tool.current_tool_id(), "pointer");
        assert!(tool.set_current_tool("scorch"));
        assert_eq!(tool.current_tool_id(), "scorch");
        assert_eq!(tool.current_tool_name(), "Scorch");
        assert_eq!(tool.tool_manager.active_tool_id(), Some("scorch"));
        assert_eq!(tool.tool_manager.get_tool_name("scorch"), Some("Scorch"));
    }

    #[test]
    fn scorch_mouse_down_only_when_scorch_is_current() {
        let mut tool = WorldBuilderTool::new().expect("WorldBuilderTool::new");
        assert!(
            tool.scorch_mouse_down(Coord3D::new(14.0, 26.0, 3.0))
                .is_none()
        );
        assert!(tool.scorch_objects().is_empty());

        assert!(tool.set_current_tool("scorch"));
        let idx = tool
            .scorch_mouse_down(Coord3D::new(14.0, 26.0, 3.0))
            .expect("place scorch");
        assert_eq!(idx, 0);
        assert_eq!(tool.scorch_objects().len(), 1);
        assert!(tool.scorch_objects()[0].is_scorch());
        assert!(tool.scorch_objects()[0].is_selected());

        let again = tool
            .scorch_mouse_down(Coord3D::new(12.0, 26.0, 3.0))
            .expect("pick existing");
        assert_eq!(again, 0);
        assert_eq!(tool.scorch_objects().len(), 1);
    }
}
