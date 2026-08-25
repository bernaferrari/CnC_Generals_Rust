//! Shared tool chrome: menu bar, tool palette, status bar, and dock layout.
//!
//! This is the testable model layer used by the egui `ToolApp` shell. Tools keep
//! implementing [`crate::GameTool`]; chrome is owned by the app and rendered
//! around the tool viewport.
//!
//! # Chrome layout
//!
//! ```text
//! +------------------------------------------------------------------+
//! | MenuBar  File | Edit | View | (tool menus) | Help          FPS   |
//! +----------+----------------------------------------+--------------+
//! |          |                                        |              |
//! | Left     |         Center viewport                | Right        |
//! | Tool     |         (GameTool::update)             | Properties   |
//! | Palette  |                                        | dock         |
//! |          |                                        |              |
//! +----------+----------------------------------------+--------------+
//! | StatusBar  message | cursor | map coords                    zoom |
//! +------------------------------------------------------------------+
//! ```

use eframe::egui;
use serde::{Deserialize, Serialize};

/// Full chrome state for a development tool window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chrome {
    pub menu_bar: MenuBar,
    pub status_bar: StatusBar,
    pub tool_palette: ToolPalette,
    pub layout: ChromeLayout,
    /// Last activated command id (menu item or palette tool).
    last_command: Option<String>,
}

impl Default for Chrome {
    fn default() -> Self {
        Self::new()
    }
}

impl Chrome {
    /// Default chrome with File/Edit/View/Help menus and a named tool palette.
    pub fn new() -> Self {
        Self {
            menu_bar: MenuBar::default_tool_menus(),
            status_bar: StatusBar::new(),
            tool_palette: ToolPalette::default_tools(),
            layout: ChromeLayout::default(),
            last_command: None,
        }
    }

    pub fn last_command(&self) -> Option<&str> {
        self.last_command.as_deref()
    }

    pub fn take_command(&mut self) -> Option<String> {
        self.last_command.take()
    }

    pub fn set_last_command(&mut self, id: impl Into<String>) {
        self.last_command = Some(id.into());
    }

    /// Render the top menu bar.
    ///
    /// - `extra` is invoked before Help so [`crate::GameTool::menu_bar`] can inject items.
    /// - `trailing` is right-aligned (FPS, etc.).
    ///
    /// Returns the activated menu item id, if any.
    pub fn show_menu_bar(
        &mut self,
        ctx: &egui::Context,
        mut extra: impl FnMut(&mut egui::Ui),
        mut trailing: impl FnMut(&mut egui::Ui),
    ) -> Option<String> {
        if !self.layout.show_menu_bar {
            return None;
        }

        let mut activated = None;
        let menus = self.menu_bar.menus.clone();
        let help_index = menus.iter().position(|m| m.id == "help");

        egui::TopBottomPanel::top("chrome_menu_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (index, menu) in menus.iter().enumerate() {
                    if help_index == Some(index) {
                        extra(ui);
                    }
                    show_menu(ui, menu, &mut activated);
                }
                if help_index.is_none() {
                    extra(ui);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    trailing(ui);
                });
            });
        });

        if let Some(id) = activated.clone() {
            self.last_command = Some(id);
        }
        activated
    }

    /// Render the bottom status bar (message, cursor/map coords, zoom).
    pub fn show_status_bar(
        &mut self,
        ctx: &egui::Context,
        mut extra_right: impl FnMut(&mut egui::Ui),
    ) {
        if !self.layout.show_status_bar {
            return;
        }

        egui::TopBottomPanel::bottom("chrome_status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.status_bar.message());

                if let Some([x, y]) = self.status_bar.cursor() {
                    ui.separator();
                    ui.label(format!("Cursor: {x:.0}, {y:.0}"));
                }

                if let Some([x, y]) = self.status_bar.map_coords() {
                    ui.separator();
                    ui.label(format!("Map: {x:.1}, {y:.1}"));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    extra_right(ui);
                    ui.label(format!("Zoom: {:.0}%", self.status_bar.zoom_percent()));
                });
            });
        });
    }

    /// Render the left tool palette list. Clicking a tool updates the selection.
    pub fn show_tool_palette(&mut self, ctx: &egui::Context) {
        if !self.layout.show_left_palette {
            return;
        }

        let width = self.layout.left_width;
        let mut selected = None;

        egui::SidePanel::left("chrome_tool_palette")
            .resizable(true)
            .default_width(width)
            .show(ctx, |ui| {
                ui.heading("Tools");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut last_group: Option<&str> = None;
                    for tool in self.tool_palette.tools() {
                        let group = tool.group.as_deref();
                        if group != last_group {
                            if let Some(name) = group {
                                ui.label(egui::RichText::new(name).weak().small());
                            }
                            last_group = group;
                        }

                        let is_selected = self.tool_palette.selected_id() == Some(tool.id.as_str());
                        let response = ui.add_enabled(
                            tool.enabled,
                            egui::Button::selectable(is_selected, &tool.name),
                        );
                        if response.clicked() {
                            selected = Some(tool.id.clone());
                        }
                    }
                });
            });

        if let Some(id) = selected {
            if self.tool_palette.select(&id) {
                self.last_command = Some(format!("tool.{id}"));
            }
        }
    }

    /// Render the right properties dock (reserved region of the chrome layout).
    pub fn show_properties_dock(&mut self, ctx: &egui::Context) {
        if !self.layout.show_right_properties {
            return;
        }

        let width = self.layout.right_width;
        egui::SidePanel::right("chrome_properties")
            .resizable(true)
            .default_width(width)
            .show(ctx, |ui| {
                ui.heading("Properties");
                ui.separator();
                ui.weak("No object selected");
            });
    }

    /// Render the full chrome shell around the remaining central viewport.
    pub fn show_shell(
        &mut self,
        ctx: &egui::Context,
        extra_menu: impl FnMut(&mut egui::Ui),
        menu_trailing: impl FnMut(&mut egui::Ui),
        extra_status_right: impl FnMut(&mut egui::Ui),
    ) -> Option<String> {
        let cmd = self.show_menu_bar(ctx, extra_menu, menu_trailing);
        self.show_status_bar(ctx, extra_status_right);
        self.show_tool_palette(ctx);
        self.show_properties_dock(ctx);
        cmd
    }
}

fn show_menu(ui: &mut egui::Ui, menu: &Menu, activated: &mut Option<String>) {
    if !menu.enabled {
        ui.add_enabled(false, egui::Button::new(&menu.label));
        return;
    }

    ui.menu_button(&menu.label, |ui| {
        for entry in &menu.entries {
            match entry {
                MenuEntry::Separator => {
                    ui.separator();
                }
                MenuEntry::Item(item) => {
                    let label = match &item.shortcut {
                        Some(shortcut) => format!("{}\t{shortcut}", item.label),
                        None => item.label.clone(),
                    };

                    let clicked = if let Some(checked) = item.checked {
                        ui.add_enabled(item.enabled, egui::Button::selectable(checked, label))
                            .clicked()
                    } else {
                        ui.add_enabled(item.enabled, egui::Button::new(label))
                            .clicked()
                    };

                    if clicked {
                        *activated = Some(item.id.clone());
                    }
                }
                MenuEntry::Submenu(sub) => {
                    show_menu(ui, sub, activated);
                }
            }
        }
    });
}

/// Top-level menu bar model. Items have stable ids and enabled flags.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MenuBar {
    menus: Vec<Menu>,
}

impl Default for MenuBar {
    fn default() -> Self {
        Self::default_tool_menus()
    }
}

impl MenuBar {
    pub fn new() -> Self {
        Self { menus: Vec::new() }
    }

    /// Standard File / Edit / View / Help menus with real items (not empty stubs).
    pub fn default_tool_menus() -> Self {
        let file = Menu::new("file", "File")
            .item(MenuItem::new("file.new", "New").shortcut("Ctrl+N"))
            .item(MenuItem::new("file.open", "Open...").shortcut("Ctrl+O"))
            .item(MenuItem::new("file.save", "Save").shortcut("Ctrl+S"))
            .item(MenuItem::new("file.save_as", "Save As...").shortcut("Ctrl+Shift+S"))
            .separator()
            .item(MenuItem::new("file.exit", "Exit").shortcut("Alt+F4"));

        let edit = Menu::new("edit", "Edit")
            .item(
                MenuItem::new("edit.undo", "Undo")
                    .shortcut("Ctrl+Z")
                    .enabled(false),
            )
            .item(
                MenuItem::new("edit.redo", "Redo")
                    .shortcut("Ctrl+Y")
                    .enabled(false),
            )
            .separator()
            .item(
                MenuItem::new("edit.cut", "Cut")
                    .shortcut("Ctrl+X")
                    .enabled(false),
            )
            .item(
                MenuItem::new("edit.copy", "Copy")
                    .shortcut("Ctrl+C")
                    .enabled(false),
            )
            .item(MenuItem::new("edit.paste", "Paste").shortcut("Ctrl+V"))
            .separator()
            .item(MenuItem::new("edit.preferences", "Preferences..."));

        let theme = Menu::new("view.theme", "Theme")
            .item(MenuItem::new("view.theme.dark", "Dark").checked(true))
            .item(MenuItem::new("view.theme.light", "Light").checked(false))
            .item(MenuItem::new("view.theme.cnc_classic", "CnC Classic").checked(false))
            .item(MenuItem::new("view.theme.modern", "Modern").checked(false));

        let view = Menu::new("view", "View")
            .submenu(theme)
            .separator()
            .item(MenuItem::new("view.zoom_in", "Zoom In").shortcut("Ctrl+="))
            .item(MenuItem::new("view.zoom_out", "Zoom Out").shortcut("Ctrl+-"))
            .item(MenuItem::new("view.zoom_reset", "Reset Zoom").shortcut("Ctrl+0"))
            .separator()
            .item(MenuItem::new("view.palette", "Tool Palette").checked(true))
            .item(MenuItem::new("view.properties", "Properties").checked(true))
            .item(MenuItem::new("view.hot_reload", "Hot Reload").checked(true));

        let help = Menu::new("help", "Help")
            .item(MenuItem::new("help.documentation", "Documentation"))
            .separator()
            .item(MenuItem::new("help.about", "About"));

        Self {
            menus: vec![file, edit, view, help],
        }
    }

    pub fn menus(&self) -> &[Menu] {
        &self.menus
    }

    pub fn menus_mut(&mut self) -> &mut Vec<Menu> {
        &mut self.menus
    }

    pub fn push_menu(&mut self, menu: Menu) {
        self.menus.push(menu);
    }

    pub fn has_menu(&self, id_or_label: &str) -> bool {
        self.menus.iter().any(|m| {
            m.id.eq_ignore_ascii_case(id_or_label) || m.label.eq_ignore_ascii_case(id_or_label)
        })
    }

    pub fn menu(&self, id: &str) -> Option<&Menu> {
        self.menus.iter().find(|m| m.id == id)
    }

    pub fn menu_mut(&mut self, id: &str) -> Option<&mut Menu> {
        self.menus.iter_mut().find(|m| m.id == id)
    }

    pub fn item(&self, id: &str) -> Option<&MenuItem> {
        self.menus.iter().find_map(|m| m.find_item(id))
    }

    pub fn item_mut(&mut self, id: &str) -> Option<&mut MenuItem> {
        self.menus.iter_mut().find_map(|m| m.find_item_mut(id))
    }

    pub fn set_item_enabled(&mut self, id: &str, enabled: bool) -> bool {
        if let Some(item) = self.item_mut(id) {
            item.enabled = enabled;
            true
        } else {
            false
        }
    }

    pub fn set_item_checked(&mut self, id: &str, checked: bool) -> bool {
        if let Some(item) = self.item_mut(id) {
            item.checked = Some(checked);
            true
        } else {
            false
        }
    }
}

/// A named top-level or nested menu.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Menu {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub entries: Vec<MenuEntry>,
}

impl Menu {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            enabled: true,
            entries: Vec::new(),
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn item(mut self, item: MenuItem) -> Self {
        self.entries.push(MenuEntry::Item(item));
        self
    }

    pub fn separator(mut self) -> Self {
        self.entries.push(MenuEntry::Separator);
        self
    }

    pub fn submenu(mut self, menu: Menu) -> Self {
        self.entries.push(MenuEntry::Submenu(menu));
        self
    }

    pub fn find_item(&self, id: &str) -> Option<&MenuItem> {
        for entry in &self.entries {
            match entry {
                MenuEntry::Item(item) if item.id == id => return Some(item),
                MenuEntry::Submenu(sub) => {
                    if let Some(item) = sub.find_item(id) {
                        return Some(item);
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub fn find_item_mut(&mut self, id: &str) -> Option<&mut MenuItem> {
        for entry in &mut self.entries {
            match entry {
                MenuEntry::Item(item) if item.id == id => return Some(item),
                MenuEntry::Submenu(sub) => {
                    if let Some(item) = sub.find_item_mut(id) {
                        return Some(item);
                    }
                }
                _ => {}
            }
        }
        None
    }
}

/// Entry in a menu: action, separator, or nested submenu.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MenuEntry {
    Item(MenuItem),
    Separator,
    Submenu(Menu),
}

/// A single actionable menu item with a stable id and enabled flag.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MenuItem {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub shortcut: Option<String>,
    pub checked: Option<bool>,
}

impl MenuItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            enabled: true,
            shortcut: None,
            checked: None,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }
}

/// Bottom status bar: message, cursor/map coordinates, zoom.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatusBar {
    message: String,
    cursor: Option<[f32; 2]>,
    map_coords: Option<[f32; 2]>,
    zoom: f32,
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            message: "Ready".to_string(),
            cursor: None,
            map_coords: None,
            zoom: 1.0,
        }
    }

    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn set_cursor(&mut self, x: f32, y: f32) {
        self.cursor = Some([x, y]);
    }

    pub fn cursor(&self) -> Option<[f32; 2]> {
        self.cursor
    }

    pub fn clear_cursor(&mut self) {
        self.cursor = None;
    }

    pub fn set_map_coords(&mut self, x: f32, y: f32) {
        self.map_coords = Some([x, y]);
    }

    pub fn map_coords(&self) -> Option<[f32; 2]> {
        self.map_coords
    }

    pub fn clear_map_coords(&mut self) {
        self.map_coords = None;
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.max(0.01);
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    pub fn zoom_percent(&self) -> f32 {
        self.zoom * 100.0
    }
}

/// Left-side tool palette: named tools with a selected id.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolPalette {
    tools: Vec<PaletteTool>,
    selected_id: Option<String>,
}

impl Default for ToolPalette {
    fn default() -> Self {
        Self::default_tools()
    }
}

impl ToolPalette {
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            selected_id: None,
        }
    }

    /// Default named editor tools (select/move/rotate/...).
    pub fn default_tools() -> Self {
        let mut palette = Self::new();
        palette.add_tool(PaletteTool::new("select", "Select").group("Transform"));
        palette.add_tool(PaletteTool::new("move", "Move").group("Transform"));
        palette.add_tool(PaletteTool::new("rotate", "Rotate").group("Transform"));
        palette.add_tool(PaletteTool::new("scale", "Scale").group("Transform"));
        palette.add_tool(PaletteTool::new("brush", "Brush").group("Terrain"));
        palette.add_tool(PaletteTool::new("height", "Height").group("Terrain"));
        palette.add_tool(PaletteTool::new("camera", "Camera").group("View"));
        let _ = palette.select("select");
        palette
    }

    pub fn add_tool(&mut self, tool: PaletteTool) {
        self.tools.push(tool);
    }

    pub fn tools(&self) -> &[PaletteTool] {
        &self.tools
    }

    pub fn tools_mut(&mut self) -> &mut Vec<PaletteTool> {
        &mut self.tools
    }

    pub fn selected_id(&self) -> Option<&str> {
        self.selected_id.as_deref()
    }

    /// Select a tool by id. Returns true if the id exists (and is enabled).
    pub fn select(&mut self, id: &str) -> bool {
        let exists = self.tools.iter().any(|t| t.id == id && t.enabled);
        if exists {
            self.selected_id = Some(id.to_string());
        }
        exists
    }

    pub fn clear_selection(&mut self) {
        self.selected_id = None;
    }

    pub fn selected_tool(&self) -> Option<&PaletteTool> {
        let id = self.selected_id.as_deref()?;
        self.tools.iter().find(|t| t.id == id)
    }
}

/// A named tool in the palette.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaletteTool {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub group: Option<String>,
}

impl PaletteTool {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            enabled: true,
            group: None,
        }
    }

    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Dock layout for the shared tool chrome.
///
/// Regions (left → right, top → bottom):
/// - **MenuBar** (top)
/// - **Left** tool palette
/// - **Center** viewport (`GameTool::update`)
/// - **Right** properties dock
/// - **StatusBar** (bottom)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChromeLayout {
    pub left_width: f32,
    pub right_width: f32,
    pub show_menu_bar: bool,
    pub show_left_palette: bool,
    pub show_right_properties: bool,
    pub show_status_bar: bool,
}

impl Default for ChromeLayout {
    fn default() -> Self {
        Self {
            left_width: 180.0,
            right_width: 240.0,
            show_menu_bar: true,
            show_left_palette: true,
            show_right_properties: true,
            show_status_bar: true,
        }
    }
}

impl ChromeLayout {
    /// Human-readable region order for hosts (egui / gpui).
    pub fn region_order() -> &'static [&'static str] {
        &[
            "menu_bar",
            "left_palette",
            "center_viewport",
            "right_properties",
            "bottom_status",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_menu_contains_file_edit_view() {
        let chrome = Chrome::default();
        assert!(
            chrome.menu_bar.has_menu("File"),
            "default chrome menu must contain File"
        );
        assert!(
            chrome.menu_bar.has_menu("Edit"),
            "default chrome menu must contain Edit"
        );
        assert!(
            chrome.menu_bar.has_menu("View"),
            "default chrome menu must contain View"
        );

        let labels: Vec<&str> = chrome
            .menu_bar
            .menus()
            .iter()
            .map(|m| m.label.as_str())
            .collect();
        assert!(labels.contains(&"File"));
        assert!(labels.contains(&"Edit"));
        assert!(labels.contains(&"View"));

        let file = chrome.menu_bar.menu("file").expect("file menu");
        assert!(file.enabled);
        assert!(file.find_item("file.new").is_some());
        assert!(file.find_item("file.open").is_some());
        assert!(file.find_item("file.save").is_some());
        assert!(file.find_item("file.exit").is_some());

        let new_item = chrome.menu_bar.item("file.new").unwrap();
        assert!(new_item.enabled);
        assert_eq!(new_item.id, "file.new");
    }

    #[test]
    fn selecting_tool_id_updates_selected() {
        let mut palette = ToolPalette::default_tools();
        assert_eq!(palette.selected_id(), Some("select"));
        assert!(palette.select("move"));
        assert_eq!(palette.selected_id(), Some("move"));
        assert_eq!(
            palette.selected_tool().map(|t| t.name.as_str()),
            Some("Move")
        );

        let mut chrome = Chrome::default();
        assert!(chrome.tool_palette.select("rotate"));
        assert_eq!(chrome.tool_palette.selected_id(), Some("rotate"));
        assert!(!chrome.tool_palette.select("does-not-exist"));
        assert_eq!(chrome.tool_palette.selected_id(), Some("rotate"));
    }

    #[test]
    fn status_message_set_get() {
        let mut status = StatusBar::default();
        assert_eq!(status.message(), "Ready");
        status.set_message("Painting terrain");
        assert_eq!(status.message(), "Painting terrain");

        status.set_cursor(12.0, 34.0);
        assert_eq!(status.cursor(), Some([12.0, 34.0]));
        status.set_map_coords(128.0, 256.0);
        assert_eq!(status.map_coords(), Some([128.0, 256.0]));
        status.set_zoom(2.0);
        assert_eq!(status.zoom(), 2.0);
        assert_eq!(status.zoom_percent(), 200.0);

        let mut chrome = Chrome::default();
        chrome.status_bar.set_message("Saved map");
        assert_eq!(chrome.status_bar.message(), "Saved map");
    }

    #[test]
    fn layout_documents_dock_regions() {
        let order = ChromeLayout::region_order();
        assert_eq!(order[0], "menu_bar");
        assert_eq!(order[1], "left_palette");
        assert_eq!(order[2], "center_viewport");
        assert_eq!(order[3], "right_properties");
        assert_eq!(order[4], "bottom_status");
    }
}
