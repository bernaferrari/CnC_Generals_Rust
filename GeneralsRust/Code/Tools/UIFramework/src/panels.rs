//! Panel system for game development tools UI

use crate::UIError;
use anyhow::Result;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Panel manager for organizing tool UI layout
pub struct PanelManager {
    panels: HashMap<String, Box<dyn Panel>>,
    layout: PanelLayout,
    docking_enabled: bool,
}

impl PanelManager {
    pub fn new() -> Self {
        Self {
            panels: HashMap::new(),
            layout: PanelLayout::default(),
            docking_enabled: true,
        }
    }

    /// Register a new panel
    pub fn register_panel(&mut self, id: String, panel: Box<dyn Panel>) {
        self.panels.insert(id, panel);
    }

    /// Show all panels in their configured layout
    pub fn show_panels(&mut self, ctx: &egui::Context) -> Result<()> {
        for (id, panel) in &mut self.panels {
            if panel.is_visible() {
                let panel_config = self
                    .layout
                    .get_panel_config(id)
                    .cloned()
                    .unwrap_or_default();

                let mut window = egui::Window::new(panel.title())
                    .resizable(panel_config.resizable)
                    .collapsible(panel_config.collapsible)
                    .default_size(panel_config.default_size);

                if let Some(pos) = panel_config.default_pos {
                    window = window.default_pos(egui::Pos2::new(pos[0], pos[1]));
                }

                window.show(ctx, |ui| panel.show_content(ui));
            }
        }
        Ok(())
    }

    /// Get mutable reference to a panel
    pub fn get_panel_mut(&mut self, id: &str) -> Option<&mut Box<dyn Panel>> {
        self.panels.get_mut(id)
    }
}

/// Trait for UI panels
pub trait Panel {
    /// Panel title
    fn title(&self) -> &str;

    /// Panel unique identifier
    fn id(&self) -> &str;

    /// Whether the panel is currently visible
    fn is_visible(&self) -> bool;

    /// Set panel visibility
    fn set_visible(&mut self, visible: bool);

    /// Show the panel content
    fn show_content(&mut self, ui: &mut egui::Ui) -> Result<()>;

    /// Panel configuration
    fn config(&self) -> PanelConfig {
        PanelConfig::default()
    }
}

/// Configuration for a panel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelConfig {
    pub resizable: bool,
    pub collapsible: bool,
    pub default_size: [f32; 2],
    pub default_pos: Option<[f32; 2]>,
    pub dockable: bool,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            resizable: true,
            collapsible: true,
            default_size: [300.0, 400.0],
            default_pos: None,
            dockable: true,
        }
    }
}

/// Layout configuration for panels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelLayout {
    panel_configs: HashMap<String, PanelConfig>,
}

impl Default for PanelLayout {
    fn default() -> Self {
        Self {
            panel_configs: HashMap::new(),
        }
    }
}

impl PanelLayout {
    pub fn get_panel_config(&self, id: &str) -> Option<&PanelConfig> {
        self.panel_configs.get(id)
    }

    pub fn set_panel_config(&mut self, id: String, config: PanelConfig) {
        self.panel_configs.insert(id, config);
    }
}

/// Properties panel for editing object properties
pub struct PropertiesPanel {
    id: String,
    title: String,
    visible: bool,
    selected_object: Option<String>,
    properties: Vec<Property>,
}

impl PropertiesPanel {
    pub fn new() -> Self {
        Self {
            id: "properties".to_string(),
            title: "Properties".to_string(),
            visible: true,
            selected_object: None,
            properties: Vec::new(),
        }
    }

    pub fn set_object(&mut self, object_id: Option<String>) {
        self.selected_object = object_id;
        self.update_properties();
    }

    fn update_properties(&mut self) {
        // TODO: Load properties for selected object
        self.properties.clear();

        if self.selected_object.is_some() {
            // Add some example properties
            self.properties.push(Property::Text {
                name: "Name".to_string(),
                value: "Object".to_string(),
                readonly: false,
            });

            self.properties.push(Property::Vector3 {
                name: "Position".to_string(),
                value: [0.0, 0.0, 0.0],
            });

            self.properties.push(Property::Float {
                name: "Scale".to_string(),
                value: 1.0,
                min: 0.1,
                max: 10.0,
            });
        }
    }
}

impl Panel for PropertiesPanel {
    fn title(&self) -> &str {
        &self.title
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn show_content(&mut self, ui: &mut egui::Ui) -> Result<()> {
        if let Some(ref object_id) = self.selected_object {
            ui.heading(format!("Object: {}", object_id));
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                for property in &mut self.properties {
                    property.show_editor(ui);
                }
            });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("No object selected");
            });
        }

        Ok(())
    }
}

/// Different types of properties that can be edited
#[derive(Debug, Clone)]
pub enum Property {
    Text {
        name: String,
        value: String,
        readonly: bool,
    },
    Float {
        name: String,
        value: f32,
        min: f32,
        max: f32,
    },
    Int {
        name: String,
        value: i32,
        min: i32,
        max: i32,
    },
    Bool {
        name: String,
        value: bool,
    },
    Vector3 {
        name: String,
        value: [f32; 3],
    },
    Color {
        name: String,
        value: [f32; 4],
    },
    Enum {
        name: String,
        options: Vec<String>,
        selected: usize,
    },
}

impl Property {
    pub fn name(&self) -> &str {
        match self {
            Property::Text { name, .. } => name,
            Property::Float { name, .. } => name,
            Property::Int { name, .. } => name,
            Property::Bool { name, .. } => name,
            Property::Vector3 { name, .. } => name,
            Property::Color { name, .. } => name,
            Property::Enum { name, .. } => name,
        }
    }

    pub fn show_editor(&mut self, ui: &mut egui::Ui) {
        match self {
            Property::Text {
                name,
                value,
                readonly,
            } => {
                ui.horizontal(|ui| {
                    ui.label(name.as_str());
                    ui.add_enabled(!*readonly, egui::TextEdit::singleline(value));
                });
            }
            Property::Float {
                name,
                value,
                min,
                max,
            } => {
                ui.horizontal(|ui| {
                    ui.label(name.as_str());
                    ui.add(egui::Slider::new(value, *min..=*max));
                });
            }
            Property::Int {
                name,
                value,
                min,
                max,
            } => {
                ui.horizontal(|ui| {
                    ui.label(name.as_str());
                    ui.add(egui::Slider::new(value, *min..=*max));
                });
            }
            Property::Bool { name, value } => {
                ui.horizontal(|ui| {
                    ui.label(name.as_str());
                    ui.checkbox(value, "");
                });
            }
            Property::Vector3 { name, value } => {
                ui.horizontal(|ui| {
                    ui.label(name.as_str());
                    ui.add(egui::DragValue::new(&mut value[0]).prefix("X:"));
                    ui.add(egui::DragValue::new(&mut value[1]).prefix("Y:"));
                    ui.add(egui::DragValue::new(&mut value[2]).prefix("Z:"));
                });
            }
            Property::Color { name, value } => {
                ui.horizontal(|ui| {
                    ui.label(name.as_str());
                    ui.color_edit_button_rgba_unmultiplied(value);
                });
            }
            Property::Enum {
                name,
                options,
                selected,
            } => {
                ui.horizontal(|ui| {
                    ui.label(name.as_str());
                    egui::ComboBox::from_id_source(name)
                        .selected_text(options.get(*selected).unwrap_or(&"None".to_string()))
                        .show_ui(ui, |ui| {
                            for (i, option) in options.iter().enumerate() {
                                ui.selectable_value(selected, i, option);
                            }
                        });
                });
            }
        }
        ui.end_row();
    }
}

/// Scene hierarchy panel
pub struct SceneHierarchyPanel {
    id: String,
    title: String,
    visible: bool,
    nodes: Vec<SceneNode>,
    selected_node: Option<String>,
}

impl SceneHierarchyPanel {
    pub fn new() -> Self {
        Self {
            id: "scene_hierarchy".to_string(),
            title: "Scene Hierarchy".to_string(),
            visible: true,
            nodes: Vec::new(),
            selected_node: None,
        }
    }

    pub fn add_node(&mut self, node: SceneNode) {
        self.nodes.push(node);
    }

    pub fn remove_node(&mut self, id: &str) {
        self.nodes.retain(|node| node.id != id);
    }

    pub fn selected_node(&self) -> Option<&str> {
        self.selected_node.as_deref()
    }

    fn show_node(&mut self, ui: &mut egui::Ui, node: &SceneNode, level: usize) {
        let indent = (level as f32) * 20.0;

        ui.horizontal(|ui| {
            ui.add_space(indent);

            let response =
                ui.selectable_label(self.selected_node.as_ref() == Some(&node.id), &node.name);

            if response.clicked() {
                self.selected_node = Some(node.id.clone());
            }
        });

        // Show children
        for child in &node.children {
            self.show_node(ui, child, level + 1);
        }
    }
}

impl Panel for SceneHierarchyPanel {
    fn title(&self) -> &str {
        &self.title
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn show_content(&mut self, ui: &mut egui::Ui) -> Result<()> {
        egui::ScrollArea::vertical().show(ui, |ui| {
            for node in self.nodes.clone() {
                // Clone to avoid borrowing issues
                self.show_node(ui, &node, 0);
            }
        });

        Ok(())
    }
}

/// Node in the scene hierarchy
#[derive(Debug, Clone)]
pub struct SceneNode {
    pub id: String,
    pub name: String,
    pub node_type: NodeType,
    pub children: Vec<SceneNode>,
    pub visible: bool,
}

#[derive(Debug, Clone)]
pub enum NodeType {
    Object,
    Group,
    Light,
    Camera,
}
