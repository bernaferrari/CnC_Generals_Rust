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

/// Draw command emitted by [`TabControl`] for the UI renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabControlRenderCommand {
    Background {
        rect: Rect,
        color: Color,
    },
    TabBorder {
        rect: Rect,
        index: usize,
        color: Color,
        border_width: u32,
    },
    TabFill {
        rect: Rect,
        index: usize,
        color: Color,
        active: bool,
        disabled: bool,
        hovered: bool,
    },
    TabText {
        rect: Rect,
        index: usize,
        text: String,
        color: Color,
    },
    TabIcon {
        rect: Rect,
        index: usize,
        image_path: String,
    },
    ContentBorder {
        rect: Rect,
        color: Color,
        border_width: u32,
    },
    FocusOutline {
        rect: Rect,
        color: Color,
    },
}

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

    pub fn set_active_tab_index_silent(&mut self, index: usize) {
        self.active_index = index;
        if let Some(tab) = self.tabs.get(index) {
            self.selected_tab = Some(tab.id);
        } else {
            self.selected_tab = Some(index as u32);
        }
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

    fn tab_render_color(
        &self,
        theme: &GadgetTheme,
        index: usize,
        tab: Option<&Tab>,
    ) -> (Color, bool, bool, bool) {
        let active = self.active_tab_index() == index;
        let disabled = !self.enabled
            || self.is_sub_pane_disabled(index)
            || tab.map(|tab| !tab.enabled || !tab.visible).unwrap_or(false);
        let hovered = self.hovered_tab == Some(index as u32);

        let color = if disabled {
            theme.disabled_color
        } else if active {
            theme.pressed_color
        } else if hovered {
            theme.hovered_color
        } else {
            theme.normal_color
        };

        (color, active, disabled, hovered)
    }

    fn tab_text_color(&self, theme: &GadgetTheme, disabled: bool) -> Color {
        if disabled {
            theme.disabled_text_color
        } else {
            theme.text_color
        }
    }

    /// Build renderer-facing commands for the current tab-control state.
    pub fn render_commands(&self, theme: &GadgetTheme) -> Vec<TabControlRenderCommand> {
        if !self.visible {
            return Vec::new();
        }

        let mut commands = vec![TabControlRenderCommand::Background {
            rect: self.bounds,
            color: if self.enabled {
                theme.normal_color
            } else {
                theme.disabled_color
            },
        }];

        let tab_count = self.tab_count().min(NUM_TAB_PANES);
        for index in 0..tab_count {
            let tab = self.tabs.get(index);
            if tab.is_some_and(|tab| !tab.visible) {
                continue;
            }

            let tab_bounds = self.get_tab_bounds(index);
            let fill_rect = Rect::new(
                tab_bounds.x + 1,
                tab_bounds.y + 1,
                tab_bounds.width.saturating_sub(2),
                tab_bounds.height.saturating_sub(2),
            );
            let (color, active, disabled, hovered) = self.tab_render_color(theme, index, tab);

            commands.push(TabControlRenderCommand::TabBorder {
                rect: tab_bounds,
                index,
                color: theme.border_color,
                border_width: theme.border_width,
            });
            commands.push(TabControlRenderCommand::TabFill {
                rect: fill_rect,
                index,
                color,
                active,
                disabled,
                hovered,
            });

            if let Some(tab) = tab {
                if let Some(icon) = tab.icon.as_ref() {
                    commands.push(TabControlRenderCommand::TabIcon {
                        rect: fill_rect,
                        index,
                        image_path: icon.clone(),
                    });
                }

                if !tab.title.is_empty() {
                    commands.push(TabControlRenderCommand::TabText {
                        rect: fill_rect,
                        index,
                        text: tab.title.clone(),
                        color: self.tab_text_color(theme, disabled),
                    });
                }
            }
        }

        commands.push(TabControlRenderCommand::ContentBorder {
            rect: self.content_bounds(),
            color: theme.border_color,
            border_width: theme.border_width,
        });

        if self.focused {
            commands.push(TabControlRenderCommand::FocusOutline {
                rect: self.bounds,
                color: theme.focused_color,
            });
        }

        commands
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

            InputEvent::MouseDown { x, y, button }
                if *button == MouseButton::Left => {
                    if let Some(tab_index) = self.tab_at_position(*x, *y) {
                        if tab_index == self.active_tab_index() {
                            return vec![GadgetMessage::Custom {
                                gadget_id: self.id,
                                data: "input_handled".to_string(),
                            }];
                        }
                        if self.select_tab_index(tab_index) {
                            return vec![GadgetMessage::ValueChanged {
                                gadget_id: self.id,
                                value: GadgetValue::Integer(tab_index as i32),
                            }];
                        }
                    }
                    return vec![GadgetMessage::Custom {
                        gadget_id: self.id,
                        data: "input_handled".to_string(),
                    }];
                }

            _ => {}
        }

        Vec::new()
    }

    fn update(&mut self, _delta_time: f32) {
        // C++ GadgetTabControl is event-driven with no per-frame update.
        // Validate selected tab still points at a valid, enabled tab.
        if let Some(selected_id) = self.selected_tab {
            let still_valid = self
                .tabs
                .iter()
                .any(|t| t.id == selected_id && t.enabled && t.visible);
            if !still_valid {
                // Fall back to the first valid tab.
                if let Some(first_valid) = self.tabs.iter().find(|t| t.enabled && t.visible) {
                    self.selected_tab = Some(first_valid.id);
                    self.active_index = self
                        .tabs
                        .iter()
                        .position(|t| t.id == first_valid.id)
                        .unwrap_or(0);
                } else {
                    self.selected_tab = None;
                    self.active_index = 0;
                }
            } else {
                // Keep active_index in sync.
                if let Some(pos) = self.tabs.iter().position(|t| t.id == selected_id) {
                    self.active_index = pos;
                }
            }
        } else if !self.tabs.is_empty() {
            // Auto-select first valid tab if none selected.
            if let Some(first_valid) = self.tabs.iter().find(|t| t.enabled && t.visible) {
                self.selected_tab = Some(first_valid.id);
                self.active_index = self
                    .tabs
                    .iter()
                    .position(|t| t.id == first_valid.id)
                    .unwrap_or(0);
            }
        }
    }

    fn render(&self, theme: &GadgetTheme) {
        if !self.visible {
            return;
        }

        let _commands = self.render_commands(theme);
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

    #[test]
    fn left_down_switches_tabs_like_cpp() {
        let mut tab_control = TabControl::new(1, 10, 20, 400, 300);
        tab_control.set_tab_data(TabControlData {
            tab_edge: TP_TOP_SIDE,
            tab_orientation: TP_TOPLEFT,
            tab_width: 100,
            tab_height: 30,
            tab_count: 2,
            ..Default::default()
        });

        let down = tab_control.handle_input(&InputEvent::MouseDown {
            x: 150,
            y: 25,
            button: MouseButton::Left,
        });
        assert_eq!(tab_control.active_tab_index(), 1);
        assert_eq!(down.len(), 1);

        let up = tab_control.handle_input(&InputEvent::MouseUp {
            x: 25,
            y: 25,
            button: MouseButton::Left,
        });
        assert_eq!(tab_control.active_tab_index(), 1);
        assert!(up.is_empty());
    }

    #[test]
    fn left_down_on_active_or_blank_tab_area_is_handled_without_switch_like_cpp() {
        let mut tab_control = TabControl::new(1, 10, 20, 400, 300);
        tab_control.set_tab_data(TabControlData {
            tab_edge: TP_TOP_SIDE,
            tab_orientation: TP_TOPLEFT,
            tab_width: 100,
            tab_height: 30,
            tab_count: 2,
            ..Default::default()
        });
        tab_control.set_active_tab_index_silent(0);

        let active = tab_control.handle_input(&InputEvent::MouseDown {
            x: 25,
            y: 25,
            button: MouseButton::Left,
        });
        assert_eq!(tab_control.active_tab_index(), 0);
        assert!(matches!(
            active.as_slice(),
            [GadgetMessage::Custom { data, .. }] if data == "input_handled"
        ));

        let blank = tab_control.handle_input(&InputEvent::MouseDown {
            x: 350,
            y: 25,
            button: MouseButton::Left,
        });
        assert_eq!(tab_control.active_tab_index(), 0);
        assert!(matches!(
            blank.as_slice(),
            [GadgetMessage::Custom { data, .. }] if data == "input_handled"
        ));
    }

    #[test]
    fn tabcontrol_render_commands_cover_tab_geometry_states_text_and_icon() {
        let theme = GadgetTheme::default();
        let mut disabled_panes = [false; NUM_TAB_PANES];
        disabled_panes[2] = true;

        let mut tab_control = TabControl::new(1, 10, 20, 400, 300);
        tab_control.set_tab_data(TabControlData {
            tab_edge: TP_TOP_SIDE,
            tab_orientation: TP_TOPLEFT,
            tab_width: 100,
            tab_height: 30,
            tab_count: 3,
            pane_border: 5,
            sub_pane_disabled: disabled_panes,
        });
        tab_control.add_tab(Tab::new(10, "One").with_icon("one.dds"));
        tab_control.add_tab(Tab::new(20, "Two"));
        tab_control.add_tab(Tab::new(30, "Three"));
        tab_control.select_tab(20);

        let messages = tab_control.handle_input(&InputEvent::MouseMove { x: 20, y: 30 });
        assert!(messages.is_empty());

        assert_eq!(
            tab_control.render_commands(&theme),
            vec![
                TabControlRenderCommand::Background {
                    rect: Rect::new(10, 20, 400, 300),
                    color: theme.normal_color,
                },
                TabControlRenderCommand::TabBorder {
                    rect: Rect::new(15, 25, 100, 30),
                    index: 0,
                    color: theme.border_color,
                    border_width: theme.border_width,
                },
                TabControlRenderCommand::TabFill {
                    rect: Rect::new(16, 26, 98, 28),
                    index: 0,
                    color: theme.hovered_color,
                    active: false,
                    disabled: false,
                    hovered: true,
                },
                TabControlRenderCommand::TabIcon {
                    rect: Rect::new(16, 26, 98, 28),
                    index: 0,
                    image_path: "one.dds".to_string(),
                },
                TabControlRenderCommand::TabText {
                    rect: Rect::new(16, 26, 98, 28),
                    index: 0,
                    text: "One".to_string(),
                    color: theme.text_color,
                },
                TabControlRenderCommand::TabBorder {
                    rect: Rect::new(115, 25, 100, 30),
                    index: 1,
                    color: theme.border_color,
                    border_width: theme.border_width,
                },
                TabControlRenderCommand::TabFill {
                    rect: Rect::new(116, 26, 98, 28),
                    index: 1,
                    color: theme.pressed_color,
                    active: true,
                    disabled: false,
                    hovered: false,
                },
                TabControlRenderCommand::TabText {
                    rect: Rect::new(116, 26, 98, 28),
                    index: 1,
                    text: "Two".to_string(),
                    color: theme.text_color,
                },
                TabControlRenderCommand::TabBorder {
                    rect: Rect::new(215, 25, 100, 30),
                    index: 2,
                    color: theme.border_color,
                    border_width: theme.border_width,
                },
                TabControlRenderCommand::TabFill {
                    rect: Rect::new(216, 26, 98, 28),
                    index: 2,
                    color: theme.disabled_color,
                    active: false,
                    disabled: true,
                    hovered: false,
                },
                TabControlRenderCommand::TabText {
                    rect: Rect::new(216, 26, 98, 28),
                    index: 2,
                    text: "Three".to_string(),
                    color: theme.disabled_text_color,
                },
                TabControlRenderCommand::ContentBorder {
                    rect: Rect::new(15, 55, 390, 260),
                    color: theme.border_color,
                    border_width: theme.border_width,
                },
            ]
        );
    }

    #[test]
    fn tabcontrol_render_commands_cover_data_only_right_edge_focus_and_hidden() {
        let theme = GadgetTheme::default();
        let mut tab_control = TabControl::new(1, 10, 20, 240, 180);
        tab_control.set_tab_data(TabControlData {
            tab_edge: TP_RIGHT_SIDE,
            tab_orientation: TP_CENTER,
            tab_width: 40,
            tab_height: 30,
            tab_count: 2,
            pane_border: 5,
            ..Default::default()
        });
        tab_control.set_active_tab_index_silent(1);
        tab_control.set_focus(true);

        assert_eq!(
            tab_control.render_commands(&theme),
            vec![
                TabControlRenderCommand::Background {
                    rect: Rect::new(10, 20, 240, 180),
                    color: theme.normal_color,
                },
                TabControlRenderCommand::TabBorder {
                    rect: Rect::new(205, 80, 40, 30),
                    index: 0,
                    color: theme.border_color,
                    border_width: theme.border_width,
                },
                TabControlRenderCommand::TabFill {
                    rect: Rect::new(206, 81, 38, 28),
                    index: 0,
                    color: theme.normal_color,
                    active: false,
                    disabled: false,
                    hovered: false,
                },
                TabControlRenderCommand::TabBorder {
                    rect: Rect::new(205, 110, 40, 30),
                    index: 1,
                    color: theme.border_color,
                    border_width: theme.border_width,
                },
                TabControlRenderCommand::TabFill {
                    rect: Rect::new(206, 111, 38, 28),
                    index: 1,
                    color: theme.pressed_color,
                    active: true,
                    disabled: false,
                    hovered: false,
                },
                TabControlRenderCommand::ContentBorder {
                    rect: Rect::new(15, 25, 190, 170),
                    color: theme.border_color,
                    border_width: theme.border_width,
                },
                TabControlRenderCommand::FocusOutline {
                    rect: Rect::new(10, 20, 240, 180),
                    color: theme.focused_color,
                },
            ]
        );

        tab_control.set_visible(false);
        assert!(tab_control.render_commands(&theme).is_empty());
    }
}
