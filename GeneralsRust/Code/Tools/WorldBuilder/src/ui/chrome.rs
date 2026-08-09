//! WorldBuilder editor chrome (C++ `MainFrm` / `IDR_MAPDOC` / `IDR_MAINFRAME`).
//!
//! GPU-free menu, tool palette, and status-bar model. Selecting a palette or
//! Tools-menu entry updates [`EditorChrome::selected_tool_id`], which
//! `WorldBuilderTool` applies to the live tool manager.

/// C++ toolbar / `Tool::m_toolID` values from `resource.h`.
pub const ID_BRUSH_TOOL: u32 = 32771;
pub const ID_FEATHERTOOL: u32 = 32791;
pub const ID_BIG_TILE_TOOL: u32 = 32792;
pub const ID_BRUSH_ADD_TOOL: u32 = 32900;
pub const ID_BRUSH_SUBTRACT_TOOL: u32 = 32901;
pub const ID_TILE_TOOL: u32 = 32902;
pub const ID_TILE_FLOOD_FILL: u32 = 32903;
pub const ID_AUTO_EDGE_OUT_TOOL: u32 = 32905;
pub const ID_EYEDROPPER_TOOL: u32 = 32913;
pub const ID_PLACE_OBJECT_TOOL: u32 = 32918;
pub const ID_POINTER_TOOL: u32 = 32921;
pub const ID_BLEND_EDGE_TOOL: u32 = 32922;
pub const ID_GROVE_TOOL: u32 = 32924;
pub const ID_ROAD_TOOL: u32 = 32937;
pub const ID_MOLD_TOOL: u32 = 32955;
pub const ID_RULER_TOOL: u32 = 32958;
pub const ID_WAYPOINT_TOOL: u32 = 32964;
pub const ID_POLYGON_TOOL: u32 = 32968;
pub const ID_BUILD_LIST_TOOL: u32 = 32972;
pub const ID_HAND_SCROLL_TOOL: u32 = 32973;
pub const ID_FENCE_TOOL: u32 = 32979;
pub const ID_WATER_TOOL: u32 = 32986;
pub const ID_SCORCH_TOOL: u32 = 33007;
pub const ID_BORDERTOOL: u32 = 33330;
pub const ID_RAMP: u32 = 61467;

/// One C++ WorldBuilder view tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WbToolId {
    Pointer,
    Brush,
    BrushAdd,
    BrushSubtract,
    Feather,
    Tile,
    BigTile,
    Eyedropper,
    FloodFill,
    AutoEdgeOut,
    BlendEdge,
    MeshMold,
    Water,
    Object,
    Road,
    Grove,
    Ramp,
    Scorch,
    Fence,
    BuildList,
    Waypoint,
    Polygon,
    Border,
    Ruler,
    HandScroll,
}

impl WbToolId {
    /// Stable chrome / `ToolManager` id (`"scorch"`, `"pointer"`, …).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pointer => "pointer",
            Self::Brush => "brush",
            Self::BrushAdd => "brush_add",
            Self::BrushSubtract => "brush_subtract",
            Self::Feather => "feather",
            Self::Tile => "tile",
            Self::BigTile => "big_tile",
            Self::Eyedropper => "eyedropper",
            Self::FloodFill => "flood_fill",
            Self::AutoEdgeOut => "auto_edge_out",
            Self::BlendEdge => "blend_edge",
            Self::MeshMold => "mesh_mold",
            Self::Water => "water",
            Self::Object => "object",
            Self::Road => "road",
            Self::Grove => "grove",
            Self::Ramp => "ramp",
            Self::Scorch => "scorch",
            Self::Fence => "fence",
            Self::BuildList => "build_list",
            Self::Waypoint => "waypoint",
            Self::Polygon => "polygon",
            Self::Border => "border",
            Self::Ruler => "ruler",
            Self::HandScroll => "hand_scroll",
        }
    }

    /// Display name used in Tools menu, palette, and status bar.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Pointer => "Pointer",
            Self::Brush => "Brush",
            Self::BrushAdd => "Mound",
            Self::BrushSubtract => "Dig",
            Self::Feather => "Feather",
            Self::Tile => "Tile",
            Self::BigTile => "Big Tile",
            Self::Eyedropper => "Eyedropper",
            Self::FloodFill => "Flood Fill",
            Self::AutoEdgeOut => "Auto Edge Out",
            Self::BlendEdge => "Blend Edge",
            Self::MeshMold => "Mesh Mold",
            Self::Water => "Water",
            Self::Object => "Object",
            Self::Road => "Road",
            Self::Grove => "Grove",
            Self::Ramp => "Ramp",
            Self::Scorch => "Scorch",
            Self::Fence => "Fence",
            Self::BuildList => "Build List",
            Self::Waypoint => "Waypoint",
            Self::Polygon => "Polygon",
            Self::Border => "Border",
            Self::Ruler => "Ruler",
            Self::HandScroll => "Hand Scroll",
        }
    }

    /// C++ `ID_*_TOOL` resource id.
    pub fn resource_id(self) -> u32 {
        match self {
            Self::Pointer => ID_POINTER_TOOL,
            Self::Brush => ID_BRUSH_TOOL,
            Self::BrushAdd => ID_BRUSH_ADD_TOOL,
            Self::BrushSubtract => ID_BRUSH_SUBTRACT_TOOL,
            Self::Feather => ID_FEATHERTOOL,
            Self::Tile => ID_TILE_TOOL,
            Self::BigTile => ID_BIG_TILE_TOOL,
            Self::Eyedropper => ID_EYEDROPPER_TOOL,
            Self::FloodFill => ID_TILE_FLOOD_FILL,
            Self::AutoEdgeOut => ID_AUTO_EDGE_OUT_TOOL,
            Self::BlendEdge => ID_BLEND_EDGE_TOOL,
            Self::MeshMold => ID_MOLD_TOOL,
            Self::Water => ID_WATER_TOOL,
            Self::Object => ID_PLACE_OBJECT_TOOL,
            Self::Road => ID_ROAD_TOOL,
            Self::Grove => ID_GROVE_TOOL,
            Self::Ramp => ID_RAMP,
            Self::Scorch => ID_SCORCH_TOOL,
            Self::Fence => ID_FENCE_TOOL,
            Self::BuildList => ID_BUILD_LIST_TOOL,
            Self::Waypoint => ID_WAYPOINT_TOOL,
            Self::Polygon => ID_POLYGON_TOOL,
            Self::Border => ID_BORDERTOOL,
            Self::Ruler => ID_RULER_TOOL,
            Self::HandScroll => ID_HAND_SCROLL_TOOL,
        }
    }

    pub fn from_str_id(id: &str) -> Option<Self> {
        match id {
            "pointer" => Some(Self::Pointer),
            "brush" => Some(Self::Brush),
            "brush_add" | "mound" => Some(Self::BrushAdd),
            "brush_subtract" | "dig" => Some(Self::BrushSubtract),
            "feather" => Some(Self::Feather),
            "tile" => Some(Self::Tile),
            "big_tile" => Some(Self::BigTile),
            "eyedropper" => Some(Self::Eyedropper),
            "flood_fill" => Some(Self::FloodFill),
            "auto_edge_out" => Some(Self::AutoEdgeOut),
            "blend_edge" => Some(Self::BlendEdge),
            "mesh_mold" => Some(Self::MeshMold),
            "water" => Some(Self::Water),
            "object" | "object_place" => Some(Self::Object),
            "road" => Some(Self::Road),
            "grove" => Some(Self::Grove),
            "ramp" => Some(Self::Ramp),
            "scorch" => Some(Self::Scorch),
            "fence" => Some(Self::Fence),
            "build_list" => Some(Self::BuildList),
            "waypoint" => Some(Self::Waypoint),
            "polygon" => Some(Self::Polygon),
            "border" => Some(Self::Border),
            "ruler" => Some(Self::Ruler),
            "hand_scroll" => Some(Self::HandScroll),
            _ => None,
        }
    }

    /// C++ `IDR_MAINFRAME` toolbar order (skipping file/edit/script buttons).
    pub fn toolbar_order() -> &'static [WbToolId] {
        &[
            Self::Ruler,
            Self::Pointer,
            Self::Brush,
            Self::BrushAdd,
            Self::BrushSubtract,
            Self::Feather,
            Self::MeshMold,
            Self::Water,
            Self::Tile,
            Self::BigTile,
            Self::Eyedropper,
            Self::FloodFill,
            Self::AutoEdgeOut,
            Self::BlendEdge,
            Self::Object,
            Self::Road,
            Self::Grove,
            Self::Ramp,
            Self::Scorch,
            Self::Fence,
            Self::BuildList,
            Self::Waypoint,
            Self::Polygon,
            Self::Border,
        ]
    }

    /// Tools-menu subset required for chrome parity (plus remaining tools).
    pub fn tools_menu_primary() -> &'static [WbToolId] {
        &[
            Self::Brush,
            Self::Object,
            Self::Waypoint,
            Self::Road,
            Self::Scorch,
        ]
    }
}

/// Menu bar top-level entry (`&File`, `&Edit`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChromeMenu {
    pub id: &'static str,
    pub label: &'static str,
    pub items: Vec<ChromeMenuItem>,
}

/// One menu command or separator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChromeMenuItem {
    pub id: &'static str,
    pub label: &'static str,
    pub shortcut: Option<&'static str>,
    pub separator: bool,
    pub tool: Option<WbToolId>,
}

impl ChromeMenuItem {
    fn command(id: &'static str, label: &'static str, shortcut: Option<&'static str>) -> Self {
        Self {
            id,
            label,
            shortcut,
            separator: false,
            tool: None,
        }
    }

    fn tool(tool: WbToolId) -> Self {
        Self {
            id: tool.as_str(),
            label: tool.display_name(),
            shortcut: None,
            separator: false,
            tool: Some(tool),
        }
    }

    fn separator() -> Self {
        Self {
            id: "separator",
            label: "",
            shortcut: None,
            separator: true,
            tool: None,
        }
    }

    pub fn display_label(&self) -> String {
        if self.separator {
            return String::new();
        }
        match self.shortcut {
            Some(shortcut) => format!("{}\t{}", self.label, shortcut),
            None => self.label.to_string(),
        }
    }
}

/// Pending chrome action (menu, palette, or shortcut).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChromeCommand {
    FileNew,
    FileOpen,
    FileSave,
    FileSaveAs,
    FileExit,
    EditUndo,
    EditRedo,
    ViewToggle(ViewToggle),
    SelectTool(WbToolId),
    HelpAbout,
}

/// View-menu toggles matching C++ `IDR_MAPDOC` `&View`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewToggle {
    ShowGrid,
    ShowTexture,
    ShowTerrain,
    ShowObjectIcons,
    ShowWaypoints,
    ShowTriggerAreas,
    ShowShadows,
    ShowLabels,
    ShowObjects,
    ShowWireframe,
    SnapToGrid,
    BrushFeedback,
    Toolbar,
    StatusBar,
    LayersList,
}

/// Status-bar fields: map name, cell, world, current tool.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusBarState {
    pub map_name: String,
    pub cell: Option<(i32, i32)>,
    pub world: Option<(f32, f32, f32)>,
    pub tool_name: String,
    pub unsaved: bool,
}

impl Default for StatusBarState {
    fn default() -> Self {
        Self {
            map_name: "No Map Loaded".to_string(),
            cell: None,
            world: None,
            tool_name: WbToolId::Pointer.display_name().to_string(),
            unsaved: false,
        }
    }
}

impl StatusBarState {
    pub fn summary(&self) -> String {
        let cell = match self.cell {
            Some((x, y)) => format!("Cell: ({x}, {y})"),
            None => "Cell: --".to_string(),
        };
        let world = match self.world {
            Some((x, y, z)) => format!("World: ({x:.2}, {y:.2}, {z:.2})"),
            None => "World: --".to_string(),
        };
        format!(
            "Map: {} | {} | {} | Tool: {}",
            self.map_name, cell, world, self.tool_name
        )
    }
}

/// GPU-free WorldBuilder chrome: menus, palette, selected tool, status.
#[derive(Debug, Clone)]
pub struct EditorChrome {
    menus: Vec<ChromeMenu>,
    selected_tool: WbToolId,
    status: StatusBarState,
    pending: Option<ChromeCommand>,
    pub show_grid: bool,
    pub show_texture: bool,
    pub show_terrain: bool,
    pub show_object_icons: bool,
    pub show_waypoints: bool,
    pub show_trigger_areas: bool,
    pub show_shadows: bool,
    pub show_labels: bool,
    pub show_objects: bool,
    pub show_wireframe: bool,
    pub snap_to_grid: bool,
    pub brush_feedback: bool,
    pub show_toolbar: bool,
    pub show_status_bar: bool,
    pub show_layers_list: bool,
    pub show_about: bool,
}

impl Default for EditorChrome {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorChrome {
    pub fn new() -> Self {
        let mut chrome = Self {
            menus: build_main_menus(),
            selected_tool: WbToolId::Pointer,
            status: StatusBarState::default(),
            pending: None,
            show_grid: true,
            show_texture: true,
            show_terrain: true,
            show_object_icons: true,
            show_waypoints: true,
            show_trigger_areas: true,
            show_shadows: false,
            show_labels: true,
            show_objects: true,
            show_wireframe: false,
            snap_to_grid: false,
            brush_feedback: true,
            show_toolbar: true,
            show_status_bar: true,
            show_layers_list: false,
            show_about: false,
        };
        chrome.sync_status_tool_name();
        chrome
    }

    pub fn menus(&self) -> &[ChromeMenu] {
        &self.menus
    }

    pub fn palette_tools(&self) -> &'static [WbToolId] {
        WbToolId::toolbar_order()
    }

    pub fn selected_tool(&self) -> WbToolId {
        self.selected_tool
    }

    pub fn selected_tool_id(&self) -> &'static str {
        self.selected_tool.as_str()
    }

    pub fn selected_tool_name(&self) -> &'static str {
        self.selected_tool.display_name()
    }

    pub fn status(&self) -> &StatusBarState {
        &self.status
    }

    pub fn status_mut(&mut self) -> &mut StatusBarState {
        &mut self.status
    }

    /// True if the named menu contains an item whose id or label matches `item`.
    pub fn menu_contains(&self, menu: &str, item: &str) -> bool {
        let menu_key = strip_menu_markup(menu);
        let item_key = strip_menu_markup(item);
        self.menus.iter().any(|m| {
            (strip_menu_markup(m.label) == menu_key || strip_menu_markup(m.id) == menu_key)
                && m.items.iter().any(|entry| {
                    !entry.separator
                        && (strip_menu_markup(entry.label).contains(&item_key)
                            || strip_menu_markup(entry.id).contains(&item_key))
                })
        })
    }

    pub fn select_tool(&mut self, tool: WbToolId) {
        self.selected_tool = tool;
        self.sync_status_tool_name();
        self.pending = Some(ChromeCommand::SelectTool(tool));
    }

    /// Select by chrome id (`"scorch"`). Returns false if unknown.
    pub fn select_tool_id(&mut self, tool_id: &str) -> bool {
        if let Some(tool) = WbToolId::from_str_id(tool_id) {
            self.select_tool(tool);
            true
        } else {
            false
        }
    }

    pub fn queue_command(&mut self, command: ChromeCommand) {
        if let ChromeCommand::SelectTool(tool) = command {
            self.select_tool(tool);
            return;
        }
        self.pending = Some(command);
    }

    pub fn take_command(&mut self) -> Option<ChromeCommand> {
        self.pending.take()
    }

    pub fn set_map_name(&mut self, name: impl Into<String>) {
        self.status.map_name = name.into();
    }

    pub fn set_hover_coords(&mut self, cell: Option<(i32, i32)>, world: Option<(f32, f32, f32)>) {
        self.status.cell = cell;
        self.status.world = world;
    }

    pub fn set_unsaved(&mut self, unsaved: bool) {
        self.status.unsaved = unsaved;
    }

    pub fn apply_view_toggle(&mut self, toggle: ViewToggle) {
        match toggle {
            ViewToggle::ShowGrid => self.show_grid = !self.show_grid,
            ViewToggle::ShowTexture => self.show_texture = !self.show_texture,
            ViewToggle::ShowTerrain => self.show_terrain = !self.show_terrain,
            ViewToggle::ShowObjectIcons => self.show_object_icons = !self.show_object_icons,
            ViewToggle::ShowWaypoints => self.show_waypoints = !self.show_waypoints,
            ViewToggle::ShowTriggerAreas => self.show_trigger_areas = !self.show_trigger_areas,
            ViewToggle::ShowShadows => self.show_shadows = !self.show_shadows,
            ViewToggle::ShowLabels => self.show_labels = !self.show_labels,
            ViewToggle::ShowObjects => self.show_objects = !self.show_objects,
            ViewToggle::ShowWireframe => self.show_wireframe = !self.show_wireframe,
            ViewToggle::SnapToGrid => self.snap_to_grid = !self.snap_to_grid,
            ViewToggle::BrushFeedback => self.brush_feedback = !self.brush_feedback,
            ViewToggle::Toolbar => self.show_toolbar = !self.show_toolbar,
            ViewToggle::StatusBar => self.show_status_bar = !self.show_status_bar,
            ViewToggle::LayersList => self.show_layers_list = !self.show_layers_list,
        }
    }

    fn sync_status_tool_name(&mut self) {
        self.status.tool_name = self.selected_tool.display_name().to_string();
    }
}

fn strip_menu_markup(text: &str) -> String {
    text.chars()
        .filter(|c| *c != '&')
        .collect::<String>()
        .split('\t')
        .next()
        .unwrap_or("")
        .trim()
        .trim_end_matches("...")
        .to_ascii_lowercase()
}

fn build_main_menus() -> Vec<ChromeMenu> {
    vec![
        ChromeMenu {
            id: "file",
            label: "&File",
            items: vec![
                ChromeMenuItem::command("file.new", "&New", Some("Ctrl+N")),
                ChromeMenuItem::command("file.open", "&Open...", Some("Ctrl+O")),
                ChromeMenuItem::command("file.save", "&Save", Some("Ctrl+S")),
                ChromeMenuItem::command("file.save_as", "Save &As...", None),
                ChromeMenuItem::separator(),
                ChromeMenuItem::command("file.exit", "E&xit", None),
            ],
        },
        ChromeMenu {
            id: "edit",
            label: "&Edit",
            items: vec![
                ChromeMenuItem::command("edit.undo", "&Undo", Some("Ctrl+Z")),
                ChromeMenuItem::command("edit.redo", "&Redo", Some("Shft+Ctrl+Z")),
            ],
        },
        ChromeMenu {
            id: "view",
            label: "&View",
            items: vec![
                ChromeMenuItem::command("view.grid", "Show Grid", Some("Ctrl+G")),
                ChromeMenuItem::command("view.texture", "Show Texture", Some("Ctrl+T")),
                ChromeMenuItem::separator(),
                ChromeMenuItem::command("view.terrain", "Show Terrain", None),
                ChromeMenuItem::command("view.object_icons", "Show Object Icons", Some("Ctrl+B")),
                ChromeMenuItem::command("view.waypoints", "Show Waypoints", None),
                ChromeMenuItem::command("view.triggers", "Show Trigger Areas", None),
                ChromeMenuItem::command("view.shadows", "Show Shadows", None),
                ChromeMenuItem::command("view.labels", "Show Labels", None),
                ChromeMenuItem::command("view.objects", "Show Objects", None),
                ChromeMenuItem::command("view.wireframe", "Show Wireframe 3D View", Some("Ctrl+W")),
                ChromeMenuItem::command("view.snap", "Snap To Grid", Some("Ctrl+Shft+G")),
                ChromeMenuItem::command("view.brush_feedback", "Show Brush Feedback", None),
                ChromeMenuItem::separator(),
                ChromeMenuItem::command("view.toolbar", "&Toolbar", None),
                ChromeMenuItem::command("view.status_bar", "&Status Bar", None),
                ChromeMenuItem::command("view.layers", "Layers List", None),
            ],
        },
        ChromeMenu {
            id: "tools",
            label: "&Tools",
            items: {
                let mut items: Vec<ChromeMenuItem> = WbToolId::tools_menu_primary()
                    .iter()
                    .copied()
                    .map(ChromeMenuItem::tool)
                    .collect();
                items.push(ChromeMenuItem::separator());
                for tool in WbToolId::toolbar_order() {
                    if WbToolId::tools_menu_primary().contains(tool) {
                        continue;
                    }
                    items.push(ChromeMenuItem::tool(*tool));
                }
                items
            },
        },
        ChromeMenu {
            id: "help",
            label: "&Help",
            items: vec![ChromeMenuItem::command(
                "help.about",
                "&About World Builder...",
                None,
            )],
        },
    ]
}

pub fn command_for_menu_item(item: &ChromeMenuItem) -> Option<ChromeCommand> {
    if item.separator {
        return None;
    }
    if let Some(tool) = item.tool {
        return Some(ChromeCommand::SelectTool(tool));
    }
    Some(match item.id {
        "file.new" => ChromeCommand::FileNew,
        "file.open" => ChromeCommand::FileOpen,
        "file.save" => ChromeCommand::FileSave,
        "file.save_as" => ChromeCommand::FileSaveAs,
        "file.exit" => ChromeCommand::FileExit,
        "edit.undo" => ChromeCommand::EditUndo,
        "edit.redo" => ChromeCommand::EditRedo,
        "view.grid" => ChromeCommand::ViewToggle(ViewToggle::ShowGrid),
        "view.texture" => ChromeCommand::ViewToggle(ViewToggle::ShowTexture),
        "view.terrain" => ChromeCommand::ViewToggle(ViewToggle::ShowTerrain),
        "view.object_icons" => ChromeCommand::ViewToggle(ViewToggle::ShowObjectIcons),
        "view.waypoints" => ChromeCommand::ViewToggle(ViewToggle::ShowWaypoints),
        "view.triggers" => ChromeCommand::ViewToggle(ViewToggle::ShowTriggerAreas),
        "view.shadows" => ChromeCommand::ViewToggle(ViewToggle::ShowShadows),
        "view.labels" => ChromeCommand::ViewToggle(ViewToggle::ShowLabels),
        "view.objects" => ChromeCommand::ViewToggle(ViewToggle::ShowObjects),
        "view.wireframe" => ChromeCommand::ViewToggle(ViewToggle::ShowWireframe),
        "view.snap" => ChromeCommand::ViewToggle(ViewToggle::SnapToGrid),
        "view.brush_feedback" => ChromeCommand::ViewToggle(ViewToggle::BrushFeedback),
        "view.toolbar" => ChromeCommand::ViewToggle(ViewToggle::Toolbar),
        "view.status_bar" => ChromeCommand::ViewToggle(ViewToggle::StatusBar),
        "view.layers" => ChromeCommand::ViewToggle(ViewToggle::LayersList),
        "help.about" => ChromeCommand::HelpAbout,
        _ => return None,
    })
}

/// World units → height-map cell (C++ `getCellIndexFromCoord` XY divide).
pub fn world_to_cell(world_x: f32, world_y: f32, map_xy_factor: f32) -> (i32, i32) {
    let factor = if map_xy_factor == 0.0 {
        10.0
    } else {
        map_xy_factor
    };
    (
        (world_x / factor).floor() as i32,
        (world_y / factor).floor() as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_menu_contains_file_open() {
        let chrome = EditorChrome::new();
        assert!(
            chrome.menu_contains("File", "Open"),
            "C++ IDR_MAPDOC File menu must expose Open; menus={:?}",
            chrome
                .menus()
                .iter()
                .map(|m| (m.label, m.items.iter().map(|i| i.label).collect::<Vec<_>>()))
                .collect::<Vec<_>>()
        );
        assert!(chrome.menu_contains("File", "New"));
        assert!(chrome.menu_contains("File", "Save"));
        assert!(chrome.menu_contains("File", "Save As"));
        assert!(chrome.menu_contains("File", "Exit"));
        assert!(chrome.menu_contains("Edit", "Undo"));
        assert!(chrome.menu_contains("Edit", "Redo"));
        assert!(chrome.menu_contains("View", "Show Grid"));
        assert!(chrome.menu_contains("Tools", "Brush"));
        assert!(chrome.menu_contains("Tools", "Object"));
        assert!(chrome.menu_contains("Tools", "Waypoint"));
        assert!(chrome.menu_contains("Tools", "Road"));
        assert!(chrome.menu_contains("Tools", "Scorch"));
        assert!(chrome.menu_contains("Help", "About"));
    }

    #[test]
    fn selecting_scorch_tool_sets_current_tool() {
        let mut chrome = EditorChrome::new();
        assert_eq!(chrome.selected_tool_id(), "pointer");
        assert!(chrome.select_tool_id("scorch"));
        assert_eq!(chrome.selected_tool_id(), "scorch");
        assert_eq!(chrome.selected_tool_name(), "Scorch");
        assert_eq!(chrome.selected_tool(), WbToolId::Scorch);
        match chrome.take_command() {
            Some(ChromeCommand::SelectTool(WbToolId::Scorch)) => {}
            other => panic!("expected SelectTool(Scorch), got {other:?}"),
        }
    }

    #[test]
    fn palette_lists_cpp_toolbar_tools() {
        let chrome = EditorChrome::new();
        let ids: Vec<&str> = chrome.palette_tools().iter().map(|t| t.as_str()).collect();
        for required in [
            "pointer", "brush", "object", "waypoint", "road", "scorch", "water", "fence",
        ] {
            assert!(
                ids.contains(&required),
                "palette missing {required}: {ids:?}"
            );
        }
    }

    #[test]
    fn status_bar_includes_map_cell_world_and_tool() {
        let mut chrome = EditorChrome::new();
        chrome.set_map_name("Alpine Assault");
        chrome.set_hover_coords(Some((4, 7)), Some((40.0, 70.0, 3.0)));
        chrome.select_tool(WbToolId::Scorch);
        let text = chrome.status().summary();
        assert!(text.contains("Alpine Assault"), "{text}");
        assert!(text.contains("Cell: (4, 7)"), "{text}");
        assert!(text.contains("World: (40.00, 70.00, 3.00)"), "{text}");
        assert!(text.contains("Tool: Scorch"), "{text}");
    }

    #[test]
    fn world_to_cell_matches_map_xy_factor() {
        assert_eq!(world_to_cell(40.0, 70.0, 10.0), (4, 7));
        assert_eq!(world_to_cell(0.0, 0.0, 10.0), (0, 0));
        assert_eq!(world_to_cell(-1.0, 10.0, 10.0), (-1, 1));
    }
}
