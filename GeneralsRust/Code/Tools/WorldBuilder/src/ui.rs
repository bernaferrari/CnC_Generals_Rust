//! User interface for World Builder

use crate::map::MapSettings;
use crate::objects::ObjectManager;
use crate::terrain::TerrainEditor;
use crate::tools::{ToolCategory, ToolManager};
use anyhow::Result;
use eframe::egui;
use ui_framework::{
    widgets::{CollapsibleSection, ToolButton, ToolbarWidget},
    Viewport3D,
};

/// Main UI controller for World Builder
pub struct WorldBuilderUI {
    // UI state
    show_new_map_dialog: bool,
    new_map_settings: MapSettings,

    // Tool panels
    terrain_panel: TerrainPanel,
    object_panel: ObjectPanel,
    properties_panel: PropertiesPanel,

    // Toolbars
    main_toolbar: ToolbarWidget,
    terrain_toolbar: ToolbarWidget,
    object_toolbar: ToolbarWidget,
}

impl WorldBuilderUI {
    pub fn new() -> Self {
        let mut main_toolbar = ToolbarWidget::new();
        main_toolbar.add_tool(ToolButton::new("camera", "📷").exclusive());
        main_toolbar.add_tool(ToolButton::new("terrain", "🏔️").exclusive());
        main_toolbar.add_tool(ToolButton::new("objects", "🏠").exclusive());

        let mut terrain_toolbar = ToolbarWidget::new();
        terrain_toolbar.add_tool(ToolButton::new("raise", "⬆️").exclusive());
        terrain_toolbar.add_tool(ToolButton::new("lower", "⬇️").exclusive());
        terrain_toolbar.add_tool(ToolButton::new("smooth", "〰️").exclusive());
        terrain_toolbar.add_tool(ToolButton::new("flatten", "▬").exclusive());
        terrain_toolbar.add_tool(ToolButton::new("paint", "🎨").exclusive());

        let mut object_toolbar = ToolbarWidget::new();
        object_toolbar.add_tool(ToolButton::new("select", "👆").exclusive());
        object_toolbar.add_tool(ToolButton::new("place", "➕").exclusive());
        object_toolbar.add_tool(ToolButton::new("move", "↔️").exclusive());
        object_toolbar.add_tool(ToolButton::new("rotate", "🔄").exclusive());
        object_toolbar.add_tool(ToolButton::new("scale", "↕️").exclusive());

        Self {
            show_new_map_dialog: false,
            new_map_settings: MapSettings::default(),

            terrain_panel: TerrainPanel::new(),
            object_panel: ObjectPanel::new(),
            properties_panel: PropertiesPanel::new(),

            main_toolbar,
            terrain_toolbar,
            object_toolbar,
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    /// Show the tool panel
    pub fn show_tool_panel(
        &mut self,
        ui: &mut egui::Ui,
        tool_manager: &mut ToolManager,
        terrain_editor: &mut TerrainEditor,
        object_manager: &mut ObjectManager,
    ) {
        ui.heading("Tools");
        ui.separator();

        // Main tool categories
        if let Some(clicked_tool) = self.main_toolbar.show(ui) {
            match clicked_tool.as_str() {
                "camera" => tool_manager.set_active_tool("camera"),
                "terrain" => tool_manager.set_active_tool("terrain_raise"),
                "objects" => tool_manager.set_active_tool("object_select"),
                _ => {}
            }
        }

        ui.separator();

        // Category-specific tools
        if let Some(active_tool) = tool_manager.active_tool() {
            match active_tool.category() {
                ToolCategory::Terrain => {
                    ui.label("Terrain Tools:");
                    if let Some(clicked_tool) = self.terrain_toolbar.show(ui) {
                        let tool_id = match clicked_tool.as_str() {
                            "raise" => "terrain_raise",
                            "lower" => "terrain_lower",
                            "smooth" => "terrain_smooth",
                            "flatten" => "terrain_flatten",
                            "paint" => "terrain_paint",
                            _ => "terrain_raise",
                        };
                        tool_manager.set_active_tool(tool_id);
                    }

                    ui.separator();
                    self.terrain_panel.show(ui, terrain_editor);
                }
                ToolCategory::Objects => {
                    ui.label("Object Tools:");
                    if let Some(clicked_tool) = self.object_toolbar.show(ui) {
                        let tool_id = match clicked_tool.as_str() {
                            "select" => "object_select",
                            "place" => "object_place",
                            "move" => "object_move",
                            "rotate" => "object_rotate",
                            "scale" => "object_scale",
                            _ => "object_select",
                        };
                        tool_manager.set_active_tool(tool_id);
                    }

                    ui.separator();
                    self.object_panel.show(ui, object_manager);
                }
                _ => {
                    ui.label("Camera Navigation:");
                    ui.label("• WASD - Move camera");
                    ui.label("• Mouse wheel - Zoom");
                    ui.label("• Middle mouse - Look around");
                }
            }
        }
    }

    /// Show the properties panel
    pub fn show_properties_panel(
        &mut self,
        ui: &mut egui::Ui,
        object_manager: &mut ObjectManager,
        tool_manager: &ToolManager,
    ) {
        self.properties_panel.show(ui, object_manager, tool_manager);
    }

    /// Show viewport toolbar
    pub fn show_viewport_toolbar(
        &mut self,
        ui: &mut egui::Ui,
        viewport: &mut Viewport3D,
        tool_manager: &mut ToolManager,
    ) {
        ui.horizontal(|ui| {
            // Viewport controls
            if ui.button("Reset View").clicked() {
                viewport.set_camera(glam::Vec3::new(0.0, 50.0, 100.0), glam::Vec3::ZERO);
            }

            ui.separator();

            // Grid controls
            ui.label("Grid:");
            ui.checkbox(&mut true, "Show"); // TODO: Connect to actual grid visibility

            ui.separator();

            // Active tool display
            if let Some(active_tool) = tool_manager.active_tool() {
                ui.label(format!("Active: {}", active_tool.name()));
            }
        });
    }

    /// Show new map dialog
    pub fn show_new_map_dialog(&mut self) {
        self.show_new_map_dialog = true;
    }

    /// Process new map dialog if open
    pub fn process_new_map_dialog(&mut self, ctx: &egui::Context) -> Option<MapSettings> {
        let mut result = None;

        if self.show_new_map_dialog {
            let mut open = true;
            let mut should_close = false;

            egui::Window::new("New Map")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    egui::Grid::new("new_map_grid").show(ui, |ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut self.new_map_settings.name);
                        ui.end_row();

                        ui.label("Width:");
                        ui.add(
                            egui::DragValue::new(&mut self.new_map_settings.width).range(32..=2048),
                        );
                        ui.end_row();

                        ui.label("Height:");
                        ui.add(
                            egui::DragValue::new(&mut self.new_map_settings.height)
                                .range(32..=2048),
                        );
                        ui.end_row();

                        ui.label("Max Players:");
                        ui.add(
                            egui::DragValue::new(&mut self.new_map_settings.max_players)
                                .range(2..=8),
                        );
                        ui.end_row();

                        ui.label("Description:");
                        ui.text_edit_multiline(&mut self.new_map_settings.description);
                        ui.end_row();
                    });

                    ui.separator();

                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() {
                            result = Some(self.new_map_settings.clone());
                            should_close = true;
                        }

                        if ui.button("Cancel").clicked() {
                            should_close = true;
                        }
                    });
                });

            if should_close || !open {
                self.show_new_map_dialog = false;
            }
        }

        result
    }
}

/// Terrain editing panel
struct TerrainPanel {
    brush_section_open: bool,
    texture_section_open: bool,
}

impl TerrainPanel {
    fn new() -> Self {
        Self {
            brush_section_open: true,
            texture_section_open: true,
        }
    }

    fn show(&mut self, ui: &mut egui::Ui, terrain_editor: &mut TerrainEditor) {
        // Brush settings
        CollapsibleSection::show(ui, "Brush Settings", &mut self.brush_section_open, |ui| {
            let brush = terrain_editor.brush_mut();

            ui.add(egui::Slider::new(&mut brush.size, 1.0..=50.0).text("Size"));
            ui.add(egui::Slider::new(&mut brush.strength, 0.1..=2.0).text("Strength"));
            ui.add(egui::Slider::new(&mut brush.hardness, 0.0..=1.0).text("Hardness"));

            ui.horizontal(|ui| {
                ui.label("Falloff:");
                ui.radio_value(
                    &mut brush.falloff,
                    crate::terrain::BrushFalloff::Linear,
                    "Linear",
                );
                ui.radio_value(
                    &mut brush.falloff,
                    crate::terrain::BrushFalloff::Smooth,
                    "Smooth",
                );
                ui.radio_value(
                    &mut brush.falloff,
                    crate::terrain::BrushFalloff::Sharp,
                    "Sharp",
                );
            });
        });

        // Texture settings
        CollapsibleSection::show(ui, "Textures", &mut self.texture_section_open, |ui| {
            ui.label("Available Textures:");

            for (i, texture) in terrain_editor.textures().iter().enumerate() {
                if ui.selectable_label(false, &texture.name).clicked() {
                    // Select texture
                }
            }

            if ui.button("Add Texture...").clicked() {
                // Open texture browser
            }
        });

        // Undo/Redo
        ui.separator();
        ui.horizontal(|ui| {
            ui.add_enabled(terrain_editor.can_undo(), egui::Button::new("Undo"));
            ui.add_enabled(terrain_editor.can_redo(), egui::Button::new("Redo"));
        });
    }
}

/// Object editing panel
struct ObjectPanel {
    categories_section_open: bool,
    placement_section_open: bool,
}

impl ObjectPanel {
    fn new() -> Self {
        Self {
            categories_section_open: true,
            placement_section_open: true,
        }
    }

    fn show(&mut self, ui: &mut egui::Ui, object_manager: &mut ObjectManager) {
        // Object categories
        CollapsibleSection::show(
            ui,
            "Object Library",
            &mut self.categories_section_open,
            |ui| {
                for category in object_manager.get_categories() {
                    if ui
                        .collapsing(category.clone(), |ui| {
                            for template in object_manager.get_templates_by_category(&category) {
                                if ui.selectable_label(false, &template.name).clicked() {
                                    // Select template for placement
                                }
                            }
                        })
                        .body_returned
                        .is_some()
                    {
                        // Category expanded
                    }
                }
            },
        );

        // Placement settings
        CollapsibleSection::show(ui, "Placement", &mut self.placement_section_open, |ui| {
            ui.checkbox(object_manager.snap_to_grid_mut(), "Snap to Grid");
            if object_manager.snap_to_grid() {
                ui.add(
                    egui::Slider::new(object_manager.grid_size_mut(), 0.5..=10.0).text("Grid Size"),
                );
            }

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Gizmo:");
                ui.radio_value(
                    object_manager.gizmo_mode_mut(),
                    crate::objects::GizmoMode::Translate,
                    "Move",
                );
                ui.radio_value(
                    object_manager.gizmo_mode_mut(),
                    crate::objects::GizmoMode::Rotate,
                    "Rotate",
                );
                ui.radio_value(
                    object_manager.gizmo_mode_mut(),
                    crate::objects::GizmoMode::Scale,
                    "Scale",
                );
            });
        });

        // Selection info
        ui.separator();
        let selected_count = object_manager.selected_objects().len();
        if selected_count > 0 {
            ui.label(format!("Selected: {} objects", selected_count));

            ui.horizontal(|ui| {
                if ui.button("Delete").clicked() {
                    // Delete selected objects
                }

                if ui.button("Copy").clicked() {
                    // Copy selected objects
                }

                if ui.button("Duplicate").clicked() {
                    // Duplicate selected objects
                }
            });
        } else {
            ui.label("No objects selected");
        }
    }
}

/// Properties panel for selected objects
struct PropertiesPanel;

impl PropertiesPanel {
    fn new() -> Self {
        Self
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        object_manager: &ObjectManager,
        tool_manager: &ToolManager,
    ) {
        ui.heading("Properties");
        ui.separator();

        let selected_count = object_manager.selected_objects().len();

        if selected_count == 0 {
            ui.label("No objects selected");
        } else if selected_count == 1 {
            ui.label("Single Object Selected");

            // Show properties for single object
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label("Transform:");
                ui.indent("transform", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Position:");
                        ui.add(egui::DragValue::new(&mut 0.0f32).prefix("X:"));
                        ui.add(egui::DragValue::new(&mut 0.0f32).prefix("Y:"));
                        ui.add(egui::DragValue::new(&mut 0.0f32).prefix("Z:"));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Rotation:");
                        ui.add(egui::DragValue::new(&mut 0.0f32).prefix("X:"));
                        ui.add(egui::DragValue::new(&mut 0.0f32).prefix("Y:"));
                        ui.add(egui::DragValue::new(&mut 0.0f32).prefix("Z:"));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Scale:");
                        ui.add(egui::DragValue::new(&mut 1.0f32).prefix("X:"));
                        ui.add(egui::DragValue::new(&mut 1.0f32).prefix("Y:"));
                        ui.add(egui::DragValue::new(&mut 1.0f32).prefix("Z:"));
                    });
                });

                ui.separator();

                ui.label("Object Properties:");
                ui.indent("properties", |ui| {
                    // Show object-specific properties
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut String::from("Object"));

                    ui.label("Type:");
                    ui.label("Building");

                    ui.checkbox(&mut true, "Enabled");
                });
            });
        } else {
            ui.label(format!("Multiple Objects Selected ({})", selected_count));

            // Show common properties for multiple objects
            ui.label("Common Properties:");
            ui.indent("common", |ui| {
                ui.checkbox(&mut true, "Enabled");

                if ui.button("Align Objects").clicked() {
                    // Align selected objects
                }
            });
        }
    }
}
