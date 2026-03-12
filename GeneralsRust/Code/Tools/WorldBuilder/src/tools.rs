//! Tool system for World Builder

use crate::terrain::TerrainEditor;
use anyhow::Result;
use glam::Vec3;
use std::collections::HashMap;
use ui_framework::Viewport3D;

/// Manages editing tools in World Builder
pub struct ToolManager {
    tools: HashMap<String, Box<dyn Tool>>,
    active_tool: Option<String>,
    tool_state: ToolState,
}

impl ToolManager {
    pub fn new() -> Self {
        let mut manager = Self {
            tools: HashMap::new(),
            active_tool: None,
            tool_state: ToolState::default(),
        };

        manager.register_default_tools();
        manager
    }

    pub fn initialize(&mut self) -> Result<()> {
        for tool in self.tools.values_mut() {
            tool.initialize()?;
        }
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        if let Some(ref active_id) = self.active_tool.clone() {
            if let Some(tool) = self.tools.get_mut(active_id) {
                tool.update(&mut self.tool_state)?;
            }
        }
        Ok(())
    }

    /// Register default editing tools
    fn register_default_tools(&mut self) {
        // Terrain tools
        self.tools.insert(
            "terrain_raise".to_string(),
            Box::new(TerrainRaiseTool::new()),
        );
        self.tools.insert(
            "terrain_lower".to_string(),
            Box::new(TerrainLowerTool::new()),
        );
        self.tools.insert(
            "terrain_smooth".to_string(),
            Box::new(TerrainSmoothTool::new()),
        );
        self.tools.insert(
            "terrain_flatten".to_string(),
            Box::new(TerrainFlattenTool::new()),
        );
        self.tools.insert(
            "terrain_paint".to_string(),
            Box::new(TerrainPaintTool::new()),
        );

        // Object tools
        self.tools.insert(
            "object_select".to_string(),
            Box::new(ObjectSelectTool::new()),
        );
        self.tools
            .insert("object_place".to_string(), Box::new(ObjectPlaceTool::new()));
        self.tools
            .insert("object_move".to_string(), Box::new(ObjectMoveTool::new()));
        self.tools.insert(
            "object_rotate".to_string(),
            Box::new(ObjectRotateTool::new()),
        );
        self.tools
            .insert("object_scale".to_string(), Box::new(ObjectScaleTool::new()));

        // Camera tool
        self.tools
            .insert("camera".to_string(), Box::new(CameraTool::new()));

        // Set default active tool
        self.active_tool = Some("camera".to_string());
    }

    /// Set active tool
    pub fn set_active_tool(&mut self, tool_id: &str) {
        if self.tools.contains_key(tool_id) {
            // Deactivate current tool
            if let Some(ref current_id) = self.active_tool {
                if let Some(current_tool) = self.tools.get_mut(current_id) {
                    current_tool.deactivate();
                }
            }

            // Activate new tool
            self.active_tool = Some(tool_id.to_string());
            if let Some(new_tool) = self.tools.get_mut(tool_id) {
                new_tool.activate();
            }

            log::info!("Activated tool: {}", tool_id);
        }
    }

    /// Get active tool
    pub fn active_tool(&self) -> Option<&dyn Tool> {
        self.active_tool
            .as_ref()
            .and_then(|id| self.tools.get(id))
            .map(|tool| tool.as_ref())
    }

    /// Get mutable active tool
    pub fn active_tool_mut<'a>(&'a mut self) -> Option<&'a mut dyn Tool> {
        if let Some(id) = self.active_tool.as_ref() {
            if let Some(tool) = self.tools.get_mut(id) {
                Some(&mut **tool)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Get tool by ID
    pub fn get_tool(&self, tool_id: &str) -> Option<&dyn Tool> {
        self.tools.get(tool_id).map(|tool| tool.as_ref())
    }

    /// Get mutable tool by ID
    pub fn get_tool_mut<'a>(&'a mut self, tool_id: &str) -> Option<&'a mut dyn Tool> {
        if let Some(tool) = self.tools.get_mut(tool_id) {
            Some(&mut **tool)
        } else {
            None
        }
    }

    /// Get available tool IDs
    pub fn available_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Get tool name by ID
    pub fn get_tool_name(&self, tool_id: &str) -> Option<&str> {
        self.tools.get(tool_id).map(|tool| tool.name())
    }

    /// Get tools by category
    pub fn get_tools_by_category(&self, category: ToolCategory) -> Vec<(&String, &dyn Tool)> {
        self.tools
            .iter()
            .filter(|(_, tool)| tool.category() == category)
            .map(|(id, tool)| (id, tool.as_ref()))
            .collect()
    }
}

/// Trait for editing tools
pub trait Tool: Send + Sync {
    /// Tool name for display
    fn name(&self) -> &str;

    /// Tool description
    fn description(&self) -> &str;

    /// Tool category
    fn category(&self) -> ToolCategory;

    /// Icon path for tool
    fn icon(&self) -> Option<&str> {
        None
    }

    /// Initialize the tool
    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    /// Update the tool (called each frame)
    fn update(&mut self, state: &mut ToolState) -> Result<()> {
        Ok(())
    }

    /// Activate the tool
    fn activate(&mut self) {}

    /// Deactivate the tool
    fn deactivate(&mut self) {}

    /// Handle viewport input
    fn handle_viewport_input(
        &mut self,
        viewport: &mut Viewport3D,
        ui: &mut eframe::egui::Ui,
    ) -> Result<()> {
        Ok(())
    }

    /// Show tool-specific UI
    fn show_options_ui(&mut self, ui: &mut eframe::egui::Ui) {}

    /// Check if tool is active
    fn is_active(&self) -> bool {
        false
    }
}

/// Tool categories for organization
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolCategory {
    Terrain,
    Objects,
    Camera,
    Lighting,
    Effects,
    Scripting,
}

impl ToolCategory {
    pub fn name(&self) -> &'static str {
        match self {
            ToolCategory::Terrain => "Terrain",
            ToolCategory::Objects => "Objects",
            ToolCategory::Camera => "Camera",
            ToolCategory::Lighting => "Lighting",
            ToolCategory::Effects => "Effects",
            ToolCategory::Scripting => "Scripting",
        }
    }
}

/// Shared tool state
#[derive(Debug, Default)]
pub struct ToolState {
    pub mouse_position: Vec3,
    pub mouse_delta: Vec3,
    pub left_mouse_down: bool,
    pub right_mouse_down: bool,
    pub middle_mouse_down: bool,
    pub shift_held: bool,
    pub ctrl_held: bool,
    pub alt_held: bool,
}

// Tool implementations

/// Terrain raise tool
struct TerrainRaiseTool {
    active: bool,
}

impl TerrainRaiseTool {
    fn new() -> Self {
        Self { active: false }
    }
}

impl Tool for TerrainRaiseTool {
    fn name(&self) -> &str {
        "Raise Terrain"
    }
    fn description(&self) -> &str {
        "Raise terrain height"
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Terrain
    }
    fn icon(&self) -> Option<&str> {
        Some("icons/terrain_raise.png")
    }

    fn activate(&mut self) {
        self.active = true;
    }
    fn deactivate(&mut self) {
        self.active = false;
    }
    fn is_active(&self) -> bool {
        self.active
    }

    fn handle_viewport_input(
        &mut self,
        viewport: &mut Viewport3D,
        ui: &mut eframe::egui::Ui,
    ) -> Result<()> {
        // Handle terrain raising input
        Ok(())
    }
}

/// Terrain lower tool
struct TerrainLowerTool {
    active: bool,
}

impl TerrainLowerTool {
    fn new() -> Self {
        Self { active: false }
    }
}

impl Tool for TerrainLowerTool {
    fn name(&self) -> &str {
        "Lower Terrain"
    }
    fn description(&self) -> &str {
        "Lower terrain height"
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Terrain
    }
    fn icon(&self) -> Option<&str> {
        Some("icons/terrain_lower.png")
    }

    fn activate(&mut self) {
        self.active = true;
    }
    fn deactivate(&mut self) {
        self.active = false;
    }
    fn is_active(&self) -> bool {
        self.active
    }
}

/// Terrain smooth tool
struct TerrainSmoothTool {
    active: bool,
}

impl TerrainSmoothTool {
    fn new() -> Self {
        Self { active: false }
    }
}

impl Tool for TerrainSmoothTool {
    fn name(&self) -> &str {
        "Smooth Terrain"
    }
    fn description(&self) -> &str {
        "Smooth terrain irregularities"
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Terrain
    }
    fn icon(&self) -> Option<&str> {
        Some("icons/terrain_smooth.png")
    }

    fn activate(&mut self) {
        self.active = true;
    }
    fn deactivate(&mut self) {
        self.active = false;
    }
    fn is_active(&self) -> bool {
        self.active
    }
}

/// Terrain flatten tool
struct TerrainFlattenTool {
    active: bool,
}

impl TerrainFlattenTool {
    fn new() -> Self {
        Self { active: false }
    }
}

impl Tool for TerrainFlattenTool {
    fn name(&self) -> &str {
        "Flatten Terrain"
    }
    fn description(&self) -> &str {
        "Flatten terrain to target height"
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Terrain
    }
    fn icon(&self) -> Option<&str> {
        Some("icons/terrain_flatten.png")
    }

    fn activate(&mut self) {
        self.active = true;
    }
    fn deactivate(&mut self) {
        self.active = false;
    }
    fn is_active(&self) -> bool {
        self.active
    }
}

/// Terrain paint tool
struct TerrainPaintTool {
    active: bool,
}

impl TerrainPaintTool {
    fn new() -> Self {
        Self { active: false }
    }
}

impl Tool for TerrainPaintTool {
    fn name(&self) -> &str {
        "Paint Texture"
    }
    fn description(&self) -> &str {
        "Paint terrain textures"
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Terrain
    }
    fn icon(&self) -> Option<&str> {
        Some("icons/terrain_paint.png")
    }

    fn activate(&mut self) {
        self.active = true;
    }
    fn deactivate(&mut self) {
        self.active = false;
    }
    fn is_active(&self) -> bool {
        self.active
    }
}

/// Object select tool
struct ObjectSelectTool {
    active: bool,
}

impl ObjectSelectTool {
    fn new() -> Self {
        Self { active: false }
    }
}

impl Tool for ObjectSelectTool {
    fn name(&self) -> &str {
        "Select Objects"
    }
    fn description(&self) -> &str {
        "Select and manipulate objects"
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Objects
    }
    fn icon(&self) -> Option<&str> {
        Some("icons/object_select.png")
    }

    fn activate(&mut self) {
        self.active = true;
    }
    fn deactivate(&mut self) {
        self.active = false;
    }
    fn is_active(&self) -> bool {
        self.active
    }
}

/// Object place tool
struct ObjectPlaceTool {
    active: bool,
}

impl ObjectPlaceTool {
    fn new() -> Self {
        Self { active: false }
    }
}

impl Tool for ObjectPlaceTool {
    fn name(&self) -> &str {
        "Place Objects"
    }
    fn description(&self) -> &str {
        "Place new objects on the map"
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Objects
    }
    fn icon(&self) -> Option<&str> {
        Some("icons/object_place.png")
    }

    fn activate(&mut self) {
        self.active = true;
    }
    fn deactivate(&mut self) {
        self.active = false;
    }
    fn is_active(&self) -> bool {
        self.active
    }
}

/// Object move tool
struct ObjectMoveTool {
    active: bool,
}

impl ObjectMoveTool {
    fn new() -> Self {
        Self { active: false }
    }
}

impl Tool for ObjectMoveTool {
    fn name(&self) -> &str {
        "Move Objects"
    }
    fn description(&self) -> &str {
        "Move selected objects"
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Objects
    }
    fn icon(&self) -> Option<&str> {
        Some("icons/object_move.png")
    }

    fn activate(&mut self) {
        self.active = true;
    }
    fn deactivate(&mut self) {
        self.active = false;
    }
    fn is_active(&self) -> bool {
        self.active
    }
}

/// Object rotate tool
struct ObjectRotateTool {
    active: bool,
}

impl ObjectRotateTool {
    fn new() -> Self {
        Self { active: false }
    }
}

impl Tool for ObjectRotateTool {
    fn name(&self) -> &str {
        "Rotate Objects"
    }
    fn description(&self) -> &str {
        "Rotate selected objects"
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Objects
    }
    fn icon(&self) -> Option<&str> {
        Some("icons/object_rotate.png")
    }

    fn activate(&mut self) {
        self.active = true;
    }
    fn deactivate(&mut self) {
        self.active = false;
    }
    fn is_active(&self) -> bool {
        self.active
    }
}

/// Object scale tool
struct ObjectScaleTool {
    active: bool,
}

impl ObjectScaleTool {
    fn new() -> Self {
        Self { active: false }
    }
}

impl Tool for ObjectScaleTool {
    fn name(&self) -> &str {
        "Scale Objects"
    }
    fn description(&self) -> &str {
        "Scale selected objects"
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Objects
    }
    fn icon(&self) -> Option<&str> {
        Some("icons/object_scale.png")
    }

    fn activate(&mut self) {
        self.active = true;
    }
    fn deactivate(&mut self) {
        self.active = false;
    }
    fn is_active(&self) -> bool {
        self.active
    }
}

/// Camera tool
struct CameraTool {
    active: bool,
}

impl CameraTool {
    fn new() -> Self {
        Self { active: false }
    }
}

impl Tool for CameraTool {
    fn name(&self) -> &str {
        "Camera"
    }
    fn description(&self) -> &str {
        "Navigate and view the scene"
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Camera
    }
    fn icon(&self) -> Option<&str> {
        Some("icons/camera.png")
    }

    fn activate(&mut self) {
        self.active = true;
    }
    fn deactivate(&mut self) {
        self.active = false;
    }
    fn is_active(&self) -> bool {
        self.active
    }

    fn handle_viewport_input(
        &mut self,
        viewport: &mut Viewport3D,
        ui: &mut eframe::egui::Ui,
    ) -> Result<()> {
        // Camera navigation is handled by the viewport itself
        Ok(())
    }
}
