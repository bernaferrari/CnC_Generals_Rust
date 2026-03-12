//! TabControl UI Gadget
//!
//! Multi-page tab control with switching between content panels.

use super::*;

pub const NUM_TAB_PANES: usize = 8;

#[derive(Debug, Copy, Clone)]
pub struct TabControlData {
    pub tab_orientation: i32,
    pub tab_edge: i32,
    pub tab_width: i32,
    pub tab_height: i32,
    pub tab_count: i32,
    pub pane_border: i32,
    pub sub_pane_disabled: [bool; NUM_TAB_PANES],
}

impl Default for TabControlData {
    fn default() -> Self {
        Self {
            tab_orientation: 0,
            tab_edge: 0,
            tab_width: 0,
            tab_height: 0,
            tab_count: 0,
            pane_border: 0,
            sub_pane_disabled: [false; NUM_TAB_PANES],
        }
    }
}

/// Tab item
#[derive(Debug, Clone)]
pub struct Tab {
    pub id: u32,
    pub title: String,
    pub enabled: bool,
    pub visible: bool,
    pub icon: Option<String>,
}

impl Tab {
    pub fn new(id: u32, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            enabled: true,
            visible: true,
            icon: None,
        }
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

/// Tab selection callback
pub type TabCallback = Box<dyn Fn(u32) + Send + Sync>;

/// TabControl gadget
pub struct TabControl {
    id: GadgetId,
    bounds: Rect,
    state: GadgetState,
    enabled: bool,
    visible: bool,
    focused: bool,
    tabs: Vec<Tab>,
    selected_tab: Option<u32>,
    active_index: usize,
    tab_height: u32,
    tab_data: TabControlData,
    callback: Option<TabCallback>,
    tooltip: Option<String>,
    hovered_tab: Option<u32>,
}

impl TabControl {
    /// Create a new tab control
    pub fn new(id: GadgetId, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            id,
            bounds: Rect::new(x, y, width, height),
            state: GadgetState::Normal,
            enabled: true,
            visible: true,
            focused: false,
            tabs: Vec::new(),
            selected_tab: None,
            active_index: 0,
            tab_height: 30,
            tab_data: TabControlData::default(),
            callback: None,
            tooltip: None,
            hovered_tab: None,
        }
    }

    /// Add a tab
    pub fn add_tab(&mut self, tab: Tab) {
        let tab_id = tab.id;
        self.tabs.push(tab);

        if self.selected_tab.is_none() {
            self.selected_tab = Some(tab_id);
            self.active_index = 0;
        }
    }

    /// Remove a tab
    pub fn remove_tab(&mut self, tab_id: u32) -> bool {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == tab_id) {
            self.tabs.remove(pos);

            if self.selected_tab == Some(tab_id) {
                self.selected_tab = self.tabs.first().map(|t| t.id);
                self.active_index = 0;
            }

            true
        } else {
            false
        }
    }

    /// Select tab
    pub fn select_tab(&mut self, tab_id: u32) -> bool {
        if self.tabs.is_empty() {
            let index = tab_id as usize;
            return self.select_tab_index(index);
        }

        if let Some(index) = self
            .tabs
            .iter()
            .position(|t| t.id == tab_id && t.enabled && t.visible)
        {
            self.selected_tab = Some(tab_id);
            self.active_index = index;

            if let Some(callback) = &self.callback {
                callback(tab_id);
            }

            true
        } else {
            false
        }
    }

    pub fn select_tab_index(&mut self, index: usize) -> bool {
        if index >= self.tab_count() || self.is_sub_pane_disabled(index) {
            return false;
        }
        if !self.tabs.is_empty() {
            if let Some(tab) = self.tabs.get(index) {
                if !tab.enabled || !tab.visible {
                    return false;
                }
                self.selected_tab = Some(tab.id);
            }
        } else {
            self.selected_tab = Some(index as u32);
        }
        self.active_index = index;

        if let Some(callback) = &self.callback {
            let tab_id = self.selected_tab.unwrap_or(index as u32);
            callback(tab_id);
        }
        true
    }

    /// Get selected tab
    pub fn selected_tab(&self) -> Option<u32> {
        self.selected_tab
    }

    pub fn tab_data(&self) -> TabControlData {
        self.tab_data
    }

    pub fn set_tab_data(&mut self, data: TabControlData) {
        self.tab_data = data;
    }

    pub fn tab_height(&self) -> u32 {
        self.tab_height
    }

    /// Get tabs
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// Set callback
    pub fn with_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(u32) + Send + Sync + 'static,
    {
        self.callback = Some(Box::new(callback));
        self
    }

    /// Calculate tab bounds
    fn get_tab_bounds(&self, tab_index: usize) -> Rect {
        let layout = self.compute_tab_layout();
        let tab_x = layout.tabs_left + (layout.tab_dx * tab_index as i32);
        let tab_y = layout.tabs_top + (layout.tab_dy * tab_index as i32);
        Rect::new(
            self.bounds.x + tab_x,
            self.bounds.y + tab_y,
            layout.tab_width.max(0) as u32,
            layout.tab_height.max(0) as u32,
        )
    }

    /// Get content area bounds
    pub fn content_bounds(&self) -> Rect {
        let layout = self.compute_tab_layout();
        let mut width = self.bounds.width as i32 - (2 * layout.pane_border);
        let mut height = self.bounds.height as i32 - (2 * layout.pane_border);

        if layout.tab_edge == TP_TOP_SIDE || layout.tab_edge == TP_BOTTOM_SIDE {
            height -= layout.tab_height;
        }
        if layout.tab_edge == TP_LEFT_SIDE || layout.tab_edge == TP_RIGHT_SIDE {
            width -= layout.tab_width;
        }

        let mut x = layout.pane_border;
        let mut y = layout.pane_border;
        if layout.tab_edge == TP_LEFT_SIDE {
            x += layout.tab_width;
        }
        if layout.tab_edge == TP_TOP_SIDE {
            y += layout.tab_height;
        }

        Rect::new(
            self.bounds.x + x,
            self.bounds.y + y,
            width.max(0) as u32,
            height.max(0) as u32,
        )
    }

    /// Find tab at position
    fn tab_at_position(&self, x: i32, y: i32) -> Option<usize> {
        let count = self.tab_count();
        for index in 0..count {
            if self.is_sub_pane_disabled(index) {
                continue;
            }
            if let Some(tab) = self.tabs.get(index) {
                if !tab.visible || !tab.enabled {
                    continue;
                }
            }

            let tab_bounds = self.get_tab_bounds(index);
            if tab_bounds.contains_point(x, y) {
                return Some(index);
            }
        }

        None
    }

    pub fn tab_count(&self) -> usize {
        if self.tab_data.tab_count > 0 {
            self.tab_data.tab_count as usize
        } else if !self.tabs.is_empty() {
            self.tabs.len()
        } else {
            0
        }
    }

    pub fn is_sub_pane_disabled(&self, index: usize) -> bool {
        self.tab_data
            .sub_pane_disabled
            .get(index)
            .copied()
            .unwrap_or(false)
    }

    pub fn active_tab_index(&self) -> usize {
        if !self.tabs.is_empty() {
            if let Some(selected) = self.selected_tab {
                if let Some(pos) = self.tabs.iter().position(|tab| tab.id == selected) {
                    return pos;
                }
            }
        }
        self.active_index
    }

    pub fn tab_width_px(&self) -> i32 {
        if self.tab_data.tab_width > 0 {
            self.tab_data.tab_width
        } else {
            let count = self.tab_count().max(1) as i32;
            (self.bounds.width as i32 / count).max(0)
        }
    }

    pub fn tab_height_px(&self) -> i32 {
        if self.tab_data.tab_height > 0 {
            self.tab_data.tab_height
        } else {
            self.tab_height as i32
        }
    }

    pub fn tab_edge(&self) -> i32 {
        self.tab_data.tab_edge
    }

    pub fn tab_orientation(&self) -> i32 {
        self.tab_data.tab_orientation
    }

    pub fn pane_border(&self) -> i32 {
        self.tab_data.pane_border
    }

    fn compute_tab_layout(&self) -> TabLayout {
        let win_width = self.bounds.width as i32;
        let win_height = self.bounds.height as i32;
        let tab_count = self.tab_count().max(1) as i32;
        let tab_width = self.tab_width_px();
        let tab_height = self.tab_height_px();
        let pane_border = self.tab_data.pane_border;
        let tab_edge = self.tab_data.tab_edge;
        let tab_orientation = self.tab_data.tab_orientation;

        let mut horz_offset = 0;
        let mut vert_offset = 0;

        if tab_edge == TP_TOP_SIDE || tab_edge == TP_BOTTOM_SIDE {
            if tab_orientation == TP_CENTER {
                horz_offset = win_width - (2 * pane_border) - (tab_count * tab_width);
                horz_offset /= 2;
            } else if tab_orientation == TP_BOTTOMRIGHT {
                horz_offset = win_width - (2 * pane_border) - (tab_count * tab_width);
            }
        } else {
            if tab_orientation == TP_CENTER {
                vert_offset = win_height - (2 * pane_border) - (tab_count * tab_height);
                vert_offset /= 2;
            } else if tab_orientation == TP_BOTTOMRIGHT {
                vert_offset = win_height - (2 * pane_border) - (tab_count * tab_height);
            }
        }

        let (tabs_left, tabs_top) = if tab_edge == TP_TOP_SIDE {
            (pane_border + horz_offset, pane_border)
        } else if tab_edge == TP_BOTTOM_SIDE {
            (
                pane_border + horz_offset,
                win_height - pane_border - tab_height,
            )
        } else if tab_edge == TP_RIGHT_SIDE {
            (
                win_width - pane_border - tab_width,
                pane_border + vert_offset,
            )
        } else if tab_edge == TP_LEFT_SIDE {
            (pane_border, pane_border + vert_offset)
        } else {
            (pane_border, pane_border)
        };

        let (tab_dx, tab_dy) = if tab_edge == TP_TOP_SIDE || tab_edge == TP_BOTTOM_SIDE {
            (tab_width, 0)
        } else {
            (0, tab_height)
        };

        TabLayout {
            tab_width,
            tab_height,
            tab_dx,
            tab_dy,
            tabs_left,
            tabs_top,
            pane_border,
            tab_edge,
            tab_orientation,
            tab_count,
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct TabLayout {
    tab_width: i32,
    tab_height: i32,
    tab_dx: i32,
    tab_dy: i32,
    tabs_left: i32,
    tabs_top: i32,
    pane_border: i32,
    tab_edge: i32,
    tab_orientation: i32,
    tab_count: i32,
}

pub const TP_CENTER: i32 = 0;
pub const TP_TOPLEFT: i32 = 1;
pub const TP_BOTTOMRIGHT: i32 = 2;
pub const TP_TOP_SIDE: i32 = 3;
pub const TP_RIGHT_SIDE: i32 = 4;
pub const TP_LEFT_SIDE: i32 = 5;
pub const TP_BOTTOM_SIDE: i32 = 6;

impl Gadget for TabControl {
    fn id(&self) -> GadgetId {
        self.id
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn set_size(&mut self, width: u32, height: u32) {
        self.bounds.width = width;
        self.bounds.height = height;
    }

    fn state(&self) -> GadgetState {
        self.state
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn has_focus(&self) -> bool {
        self.focused
    }

    fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn handle_input(&mut self, event: &InputEvent) -> Vec<GadgetMessage> {
        if !self.enabled || !self.visible {
            return Vec::new();
        }

        match event {
            InputEvent::MouseMove { x, y } => {
                self.hovered_tab = self.tab_at_position(*x, *y).map(|idx| idx as u32);
            }

            InputEvent::MouseUp { x, y, button } => {
                if *button == MouseButton::Left {
                    if let Some(tab_index) = self.tab_at_position(*x, *y) {
                        if self.select_tab_index(tab_index) {
                            return vec![GadgetMessage::ValueChanged {
                                gadget_id: self.id,
                                value: GadgetValue::Integer(tab_index as i32),
                            }];
                        }
                    }
                }
            }

            _ => {}
        }

        Vec::new()
    }

    fn update(&mut self, _delta_time: f32) {}

    #[allow(unused_variables)]
    fn render(&self, theme: &GadgetTheme) {
        if !self.visible {
            return;
        }

        // Render tabs
        for (index, tab) in self.tabs.iter().enumerate() {
            if !tab.visible {
                continue;
            }

            let tab_bounds = self.get_tab_bounds(index);
            let is_selected = self.selected_tab == Some(tab.id);
            let is_hovered = if self.tabs.is_empty() {
                self.hovered_tab == Some(index as u32)
            } else {
                self.hovered_tab == Some(tab.id)
            };

            // Tab rendering code would go here
            // [Draw tab background, text, and icons]
        }

        // Render content area border
        let content_bounds = self.content_bounds();
        // [Draw content area border]
    }

    fn tooltip(&self) -> Option<&str> {
        self.tooltip.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tabcontrol_creation() {
        let tab_control = TabControl::new(1, 10, 20, 400, 300);
        assert_eq!(tab_control.tabs().len(), 0);
        assert_eq!(tab_control.selected_tab(), None);
    }

    #[test]
    fn test_add_tabs() {
        let mut tab_control = TabControl::new(1, 10, 20, 400, 300);
        tab_control.add_tab(Tab::new(1, "Tab 1"));
        tab_control.add_tab(Tab::new(2, "Tab 2"));

        assert_eq!(tab_control.tabs().len(), 2);
        assert_eq!(tab_control.selected_tab(), Some(1));
    }

    #[test]
    fn test_select_tab() {
        let mut tab_control = TabControl::new(1, 10, 20, 400, 300);
        tab_control.add_tab(Tab::new(1, "Tab 1"));
        tab_control.add_tab(Tab::new(2, "Tab 2"));

        assert!(tab_control.select_tab(2));
        assert_eq!(tab_control.selected_tab(), Some(2));
    }
}
