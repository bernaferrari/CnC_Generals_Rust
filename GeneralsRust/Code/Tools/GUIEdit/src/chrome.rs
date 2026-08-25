//! GUIEdit layout-editor chrome model (menus, toolbox, canvas selection, properties).
//!
//! C++ oracles:
//! - `Tools/GUIEdit/Resource/GUIEdit.rc` (File menu + popup gadget create list)
//! - `Tools/GUIEdit/Source/EditWindow.cpp` (default gadget sizes = N * grid)
//! - `Tools/GUIEdit/Source/Save.cpp` (`saveType` WINDOWTYPE tokens)
//! - `Tools/GUIEdit/Source/GUIEdit.cpp` (grid default 8, status bar, new/delete)

use std::path::{Path, PathBuf};

use crate::save::{
    ComboBoxDataEdit, GadgetData, ListBoxDataEdit, SaveError, SliderDataEdit, TextEntryDataEdit,
    WndLayout, WndWindow, parse_layout, save_layout,
};

/// C++ `GADGET_SIZE` (`GameClient/Gadget.h`).
pub const GADGET_SIZE: i32 = 16;

/// C++ `GUIEdit::m_gridResolution` default (`GUIEdit.cpp` ctor = 8).
pub const DEFAULT_GRID_RESOLUTION: i32 = 8;

/// File menu labels shown by the chrome shell (C++ `MENU_NEW` / `OPEN` / `SAVE` / `SAVEAS` / `EXIT`).
pub const FILE_MENU_LABELS: &[&str] = &["New", "Open", "Save", "Save As", "Exit"];

/// Edit menu labels (chrome adds Undo/Redo; C++ popup has Delete).
pub const EDIT_MENU_LABELS: &[&str] = &["Undo", "Redo", "Delete"];

/// View panel toggles requested for the layout-editor shell.
pub const VIEW_MENU_LABELS: &[&str] = &["Hierarchy", "Properties", "Toolbox", "Grid"];

/// Simple layout alignment commands.
pub const LAYOUT_MENU_LABELS: &[&str] = &["Align Left", "Align Right", "Align Top", "Align Bottom"];

/// C++ gadget types from the GUIEdit create-new popup / toolbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GadgetType {
    PushButton,
    StaticText,
    EntryField,
    Slider,
    ListBox,
    ComboBox,
    ProgressBar,
    RadioButton,
    CheckBox,
    /// Loaded `USER` / unknown windows (not in the left toolbox).
    User,
}

impl GadgetType {
    /// Left-toolbox types in C++ popup order grouping (task list).
    pub fn toolbox_types() -> &'static [GadgetType] {
        &[
            GadgetType::PushButton,
            GadgetType::StaticText,
            GadgetType::EntryField,
            GadgetType::Slider,
            GadgetType::ListBox,
            GadgetType::ComboBox,
            GadgetType::ProgressBar,
            GadgetType::RadioButton,
            GadgetType::CheckBox,
        ]
    }

    /// Display / chrome name (`PushButton`, …).
    pub fn as_str(self) -> &'static str {
        match self {
            GadgetType::PushButton => "PushButton",
            GadgetType::StaticText => "StaticText",
            GadgetType::EntryField => "EntryField",
            GadgetType::Slider => "Slider",
            GadgetType::ListBox => "ListBox",
            GadgetType::ComboBox => "ComboBox",
            GadgetType::ProgressBar => "ProgressBar",
            GadgetType::RadioButton => "RadioButton",
            GadgetType::CheckBox => "CheckBox",
            GadgetType::User => "User",
        }
    }

    /// C++ `saveType` WINDOWTYPE token.
    pub fn wnd_type(self) -> &'static str {
        match self {
            GadgetType::PushButton => "PUSHBUTTON",
            GadgetType::StaticText => "STATICTEXT",
            GadgetType::EntryField => "ENTRYFIELD",
            GadgetType::Slider => "HORZSLIDER",
            GadgetType::ListBox => "SCROLLLISTBOX",
            GadgetType::ComboBox => "COMBOBOX",
            GadgetType::ProgressBar => "PROGRESSBAR",
            GadgetType::RadioButton => "RADIOBUTTON",
            GadgetType::CheckBox => "CHECKBOX",
            GadgetType::User => "USER",
        }
    }

    /// Parse C++ WINDOWTYPE (and a few aliases) back to a gadget.
    pub fn from_wnd_type(token: &str) -> Self {
        match token.trim().to_ascii_uppercase().as_str() {
            "PUSHBUTTON" => GadgetType::PushButton,
            "STATICTEXT" => GadgetType::StaticText,
            "ENTRYFIELD" => GadgetType::EntryField,
            "HORZSLIDER" | "VERTSLIDER" | "SLIDER" => GadgetType::Slider,
            "SCROLLLISTBOX" | "LISTBOX" => GadgetType::ListBox,
            "COMBOBOX" => GadgetType::ComboBox,
            "PROGRESSBAR" => GadgetType::ProgressBar,
            "RADIOBUTTON" => GadgetType::RadioButton,
            "CHECKBOX" => GadgetType::CheckBox,
            _ => GadgetType::User,
        }
    }

    /// C++ `EditWindow` popup sizes: N * grid (default grid 8), `GADGET_SIZE` = 16.
    pub fn default_size(self, grid: i32) -> (i32, i32) {
        let g = if grid > 0 {
            grid
        } else {
            DEFAULT_GRID_RESOLUTION
        };
        match self {
            GadgetType::PushButton
            | GadgetType::CheckBox
            | GadgetType::RadioButton
            | GadgetType::ComboBox => (15 * g, 3 * g),
            GadgetType::ListBox => (20 * g, 20 * g),
            GadgetType::Slider => (20 * g, GADGET_SIZE),
            GadgetType::ProgressBar => (40 * g, GADGET_SIZE),
            GadgetType::EntryField => (20 * g, 25),
            GadgetType::StaticText | GadgetType::User => (15 * g, 15 * g),
        }
    }

    fn default_gadget_data(self) -> Option<GadgetData> {
        match self {
            GadgetType::ListBox => Some(GadgetData::ListBox(ListBoxDataEdit {
                length: 8,
                auto_scroll: 0,
                scroll_if_at_end: 0,
                auto_purge: 0,
                scroll_bar: 1,
                multi_select: 0,
                columns: 1,
                column_width_pct: Vec::new(),
                force_select: 0,
                extra_draw: Vec::new(),
            })),
            GadgetType::ComboBox => Some(GadgetData::ComboBox(ComboBoxDataEdit {
                is_editable: 1,
                max_chars: 16,
                max_display: 5,
                ascii_only: 0,
                letters_and_numbers: 0,
                extra_draw: Vec::new(),
            })),
            GadgetType::RadioButton => Some(GadgetData::RadioButton { group: 0 }),
            GadgetType::Slider => Some(GadgetData::Slider(SliderDataEdit {
                min_val: 0,
                max_val: 100,
                extra_draw: Vec::new(),
            })),
            GadgetType::StaticText => Some(GadgetData::StaticText { centered: 0 }),
            GadgetType::EntryField => Some(GadgetData::TextEntry(TextEntryDataEdit {
                max_len: 16,
                secret_text: 0,
                numerical_only: 0,
                alphanumerical_only: 0,
                ascii_only: 0,
            })),
            _ => None,
        }
    }
}

/// One in-memory layout widget (chrome list + canvas rect).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetInfo {
    pub id: u64,
    pub name: String,
    pub gadget: GadgetType,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl WidgetInfo {
    pub fn contains_point(&self, px: f32, py: f32) -> bool {
        let x0 = self.x as f32;
        let y0 = self.y as f32;
        let x1 = (self.x + self.width) as f32;
        let y1 = (self.y + self.height) as f32;
        px >= x0 && px <= x1 && py >= y0 && py <= y1
    }

    pub fn window_type(&self) -> &'static str {
        self.gadget.wnd_type()
    }
}

#[derive(Clone)]
struct Snapshot {
    widgets: Vec<WidgetInfo>,
    selected: Option<u64>,
    next_id: u64,
}

/// In-memory layout editor used by `win_main` chrome and unit tests.
#[derive(Clone)]
pub struct ChromeEditor {
    pub widgets: Vec<WidgetInfo>,
    pub selected: Option<u64>,
    pub current_path: Option<PathBuf>,
    pub show_hierarchy: bool,
    pub show_properties: bool,
    pub show_toolbox: bool,
    pub show_grid: bool,
    pub snap_to_grid: bool,
    pub grid_size: i32,
    pub zoom: f32,
    next_id: u64,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
}

impl Default for ChromeEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl ChromeEditor {
    pub fn new() -> Self {
        Self {
            widgets: Vec::new(),
            selected: None,
            current_path: None,
            show_hierarchy: true,
            show_properties: true,
            show_toolbox: true,
            show_grid: true,
            snap_to_grid: true,
            grid_size: DEFAULT_GRID_RESOLUTION,
            zoom: 1.0,
            next_id: 1,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn file_menu_labels() -> &'static [&'static str] {
        FILE_MENU_LABELS
    }

    pub fn edit_menu_labels() -> &'static [&'static str] {
        EDIT_MENU_LABELS
    }

    pub fn view_menu_labels() -> &'static [&'static str] {
        VIEW_MENU_LABELS
    }

    pub fn layout_menu_labels() -> &'static [&'static str] {
        LAYOUT_MENU_LABELS
    }

    pub fn widget_count(&self) -> usize {
        self.widgets.len()
    }

    pub fn selected_widget(&self) -> Option<&WidgetInfo> {
        let id = self.selected?;
        self.widgets.iter().find(|w| w.id == id)
    }

    pub fn selected_widget_mut(&mut self) -> Option<&mut WidgetInfo> {
        let id = self.selected?;
        self.widgets.iter_mut().find(|w| w.id == id)
    }

    pub fn selected_name(&self) -> Option<&str> {
        self.selected_widget().map(|w| w.name.as_str())
    }

    pub fn status_line(&self) -> String {
        let selected = self.selected_name().unwrap_or("(none)");
        format!(
            "Widgets: {} | Selected: {} | Grid: {}",
            self.widget_count(),
            selected,
            self.grid_size
        )
    }

    /// File → New (C++ `GUIEdit::newLayout`).
    pub fn new_layout(&mut self) {
        self.push_undo();
        self.widgets.clear();
        self.selected = None;
        self.current_path = None;
        self.next_id = 1;
        self.redo_stack.clear();
    }

    /// Toolbox click: append a gadget with C++ default size and a unique name.
    pub fn add_gadget(&mut self, gadget: GadgetType) -> u64 {
        self.push_undo();
        let count = self.widgets.iter().filter(|w| w.gadget == gadget).count();
        let (width, height) = gadget.default_size(self.grid_size);
        let cascade = ((count as i32) % 8) * self.grid_size.max(1);
        let widget = WidgetInfo {
            id: self.next_id,
            name: format!("{}{}", gadget.as_str(), count + 1),
            gadget,
            x: self.grid_size + cascade,
            y: self.grid_size + cascade,
            width,
            height,
        };
        let id = widget.id;
        self.next_id += 1;
        self.selected = Some(id);
        self.widgets.push(widget);
        id
    }

    pub fn delete_selected(&mut self) {
        let Some(id) = self.selected else {
            return;
        };
        self.push_undo();
        self.widgets.retain(|w| w.id != id);
        self.selected = None;
    }

    pub fn select(&mut self, id: Option<u64>) {
        self.selected = id.filter(|id| self.widgets.iter().any(|w| w.id == *id));
    }

    /// Hit-test canvas layout coords (already un-zoomed). Last widget wins (drawn on top).
    pub fn select_at_point(&mut self, x: f32, y: f32) {
        self.selected = self
            .widgets
            .iter()
            .rev()
            .find(|w| w.contains_point(x, y))
            .map(|w| w.id);
    }

    pub fn undo(&mut self) {
        let Some(prev) = self.undo_stack.pop() else {
            return;
        };
        self.redo_stack.push(self.snapshot());
        self.restore(prev);
    }

    pub fn redo(&mut self) {
        let Some(next) = self.redo_stack.pop() else {
            return;
        };
        self.undo_stack.push(self.snapshot());
        self.restore(next);
    }

    /// Align every widget to the selected widget's corresponding edge.
    pub fn align_left(&mut self) {
        let Some(anchor) = self.selected_widget().map(|w| w.x) else {
            return;
        };
        self.push_undo();
        for w in &mut self.widgets {
            w.x = anchor;
        }
    }

    pub fn align_right(&mut self) {
        let Some(anchor) = self.selected_widget().map(|w| w.x + w.width) else {
            return;
        };
        self.push_undo();
        for w in &mut self.widgets {
            w.x = anchor - w.width;
        }
    }

    pub fn align_top(&mut self) {
        let Some(anchor) = self.selected_widget().map(|w| w.y) else {
            return;
        };
        self.push_undo();
        for w in &mut self.widgets {
            w.y = anchor;
        }
    }

    pub fn align_bottom(&mut self) {
        let Some(anchor) = self.selected_widget().map(|w| w.y + w.height) else {
            return;
        };
        self.push_undo();
        for w in &mut self.widgets {
            w.y = anchor - w.height;
        }
    }

    pub fn apply_layout_command(&mut self, label: &str) {
        match label {
            "Align Left" => self.align_left(),
            "Align Right" => self.align_right(),
            "Align Top" => self.align_top(),
            "Align Bottom" => self.align_bottom(),
            _ => {}
        }
    }

    /// Convert chrome widgets to the shipped `.wnd` layout model.
    pub fn to_wnd_layout(&self) -> WndLayout {
        let mut layout = WndLayout::default();
        layout.windows = self.widgets.iter().map(widget_to_window).collect();
        layout
    }

    /// Replace chrome widgets from a parsed `.wnd` layout (children flattened; screen rects).
    pub fn from_wnd_layout(&mut self, layout: &WndLayout) {
        self.widgets.clear();
        self.selected = None;
        self.next_id = 1;
        let mut acc = Vec::new();
        for window in &layout.windows {
            collect_windows(window, &mut acc);
        }
        for window in acc {
            let gadget = GadgetType::from_wnd_type(&window.window_type);
            let width = (window.br_x - window.ul_x).max(1);
            let height = (window.br_y - window.ul_y).max(1);
            self.widgets.push(WidgetInfo {
                id: self.next_id,
                name: strip_wnd_name_prefix(&window.name),
                gadget,
                x: window.ul_x,
                y: window.ul_y,
                width,
                height,
            });
            self.next_id += 1;
        }
    }

    /// Serialize via shipped [`save_layout`] (C++ `.wnd` text, not a new INI format).
    pub fn save_to_string(&self) -> String {
        save_layout(&self.layout_filename_token(), &self.to_wnd_layout())
    }

    /// Parse via shipped [`parse_layout`].
    pub fn load_from_string(&mut self, text: &str) -> Result<(), SaveError> {
        let layout = parse_layout(text)?;
        self.push_undo();
        self.from_wnd_layout(&layout);
        self.redo_stack.clear();
        Ok(())
    }

    pub fn write_to_path(&mut self, path: &Path) -> std::io::Result<()> {
        self.current_path = Some(path.to_path_buf());
        std::fs::write(path, self.save_to_string())
    }

    pub fn read_from_path(&mut self, path: &Path) -> Result<(), String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        self.load_from_string(&text).map_err(|e| e.to_string())?;
        self.current_path = Some(path.to_path_buf());
        Ok(())
    }

    fn layout_filename_token(&self) -> String {
        self.current_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Untitled.wnd".to_string())
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            widgets: self.widgets.clone(),
            selected: self.selected,
            next_id: self.next_id,
        }
    }

    fn restore(&mut self, snap: Snapshot) {
        self.widgets = snap.widgets;
        self.selected = snap.selected;
        self.next_id = snap.next_id;
    }

    fn push_undo(&mut self) {
        self.undo_stack.push(self.snapshot());
        self.redo_stack.clear();
        const MAX_UNDO: usize = 64;
        if self.undo_stack.len() > MAX_UNDO {
            self.undo_stack.remove(0);
        }
    }
}

fn strip_wnd_name_prefix(name: &str) -> String {
    if let Some((prefix, rest)) = name.split_once(':') {
        if prefix.ends_with(".wnd") || prefix.contains('.') {
            return rest.to_string();
        }
    }
    name.to_string()
}

fn collect_windows(window: &WndWindow, out: &mut Vec<WndWindow>) {
    let mut clone = window.clone();
    clone.children.clear();
    out.push(clone);
    for child in &window.children {
        collect_windows(child, out);
    }
}

fn widget_to_window(widget: &WidgetInfo) -> WndWindow {
    let mut window = WndWindow::user(
        &widget.name,
        widget.x,
        widget.y,
        widget.x + widget.width,
        widget.y + widget.height,
    );
    window.window_type = widget.gadget.wnd_type().to_string();
    window.style = widget.gadget.wnd_type().to_string();
    window.text = match widget.gadget {
        GadgetType::PushButton => "Button".to_string(),
        GadgetType::CheckBox => "Check".to_string(),
        GadgetType::RadioButton => "Radio".to_string(),
        GadgetType::EntryField => "Entry".to_string(),
        GadgetType::StaticText => "Static Text".to_string(),
        _ => String::new(),
    };
    window.gadget = widget.gadget.default_gadget_data();
    window
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_push_button_increases_widget_count() {
        let mut editor = ChromeEditor::new();
        assert_eq!(editor.widget_count(), 0);
        editor.add_gadget(GadgetType::PushButton);
        assert_eq!(editor.widget_count(), 1);
        assert_eq!(editor.widgets[0].gadget, GadgetType::PushButton);
        assert_eq!(editor.widgets[0].name, "PushButton1");
        assert_eq!(editor.widgets[0].window_type(), "PUSHBUTTON");
        let (w, h) = GadgetType::PushButton.default_size(DEFAULT_GRID_RESOLUTION);
        assert_eq!(editor.widgets[0].width, w);
        assert_eq!(editor.widgets[0].height, h);
        editor.add_gadget(GadgetType::PushButton);
        assert_eq!(editor.widget_count(), 2);
    }

    #[test]
    fn save_reload_roundtrip_via_shipped_wnd_parse() {
        let mut editor = ChromeEditor::new();
        editor.add_gadget(GadgetType::PushButton);
        editor.add_gadget(GadgetType::StaticText);
        if let Some(w) = editor.selected_widget_mut() {
            w.x = 40;
            w.y = 80;
            w.width = 120;
            w.height = 24;
            w.name = "Ok".to_string();
        }
        let text = editor.save_to_string();
        assert!(
            text.contains("WINDOWTYPE = PUSHBUTTON;"),
            "must use shipped save.rs tokens, got:\n{text}"
        );
        assert!(text.contains("FILE_VERSION = 2;"));
        assert!(text.contains("STARTLAYOUTBLOCK"));
        assert!(text.contains("WINDOWTYPE = STATICTEXT;"));

        let mut reloaded = ChromeEditor::new();
        reloaded
            .load_from_string(&text)
            .expect("parse_layout from save.rs");
        assert_eq!(reloaded.widget_count(), 2);
        let ok = reloaded
            .widgets
            .iter()
            .find(|w| w.name == "Ok")
            .expect("Ok widget");
        assert_eq!(ok.gadget, GadgetType::StaticText);
        assert_eq!(ok.x, 40);
        assert_eq!(ok.y, 80);
        assert_eq!(ok.width, 120);
        assert_eq!(ok.height, 24);
        assert!(
            reloaded
                .widgets
                .iter()
                .any(|w| w.gadget == GadgetType::PushButton)
        );
    }

    #[test]
    fn chrome_model_wnd_layout_roundtrip() {
        let mut editor = ChromeEditor::new();
        editor.add_gadget(GadgetType::EntryField);
        editor.add_gadget(GadgetType::Slider);
        let layout = editor.to_wnd_layout();
        let mut other = ChromeEditor::new();
        other.from_wnd_layout(&layout);
        assert_eq!(other.widget_count(), editor.widget_count());
        assert_eq!(other.widgets[0].gadget, GadgetType::EntryField);
        assert_eq!(other.widgets[1].gadget, GadgetType::Slider);
        assert_eq!(other.widgets[0].name, editor.widgets[0].name);
        assert_eq!(other.widgets[0].x, editor.widgets[0].x);
        assert_eq!(other.widgets[0].width, editor.widgets[0].width);
    }

    #[test]
    fn file_menu_labels_present() {
        let labels = ChromeEditor::file_menu_labels();
        for expected in ["New", "Open", "Save", "Save As", "Exit"] {
            assert!(
                labels
                    .iter()
                    .any(|label| *label == expected || label.contains(expected)),
                "File menu missing {expected:?}, have {labels:?}"
            );
        }
        assert_eq!(labels, FILE_MENU_LABELS);
        assert!(EDIT_MENU_LABELS.contains(&"Undo"));
        assert!(EDIT_MENU_LABELS.contains(&"Redo"));
        assert!(EDIT_MENU_LABELS.contains(&"Delete"));
        for expected in VIEW_MENU_LABELS {
            assert!(["Hierarchy", "Properties", "Toolbox", "Grid"].contains(expected));
        }
    }

    #[test]
    fn toolbox_exposes_cpp_gadget_types() {
        let types = GadgetType::toolbox_types();
        assert_eq!(types.len(), 9);
        assert_eq!(types[0], GadgetType::PushButton);
        assert!(types.contains(&GadgetType::StaticText));
        assert!(types.contains(&GadgetType::EntryField));
        assert!(types.contains(&GadgetType::Slider));
        assert!(types.contains(&GadgetType::ListBox));
        assert!(types.contains(&GadgetType::ComboBox));
        assert!(types.contains(&GadgetType::ProgressBar));
        assert!(types.contains(&GadgetType::RadioButton));
        assert!(types.contains(&GadgetType::CheckBox));
    }

    #[test]
    fn status_line_reports_count_selection_and_grid() {
        let mut editor = ChromeEditor::new();
        editor.add_gadget(GadgetType::ListBox);
        let line = editor.status_line();
        assert!(line.contains("Widgets: 1"));
        assert!(line.contains("ListBox1"));
        assert!(line.contains(&format!("Grid: {}", DEFAULT_GRID_RESOLUTION)));
    }
}
