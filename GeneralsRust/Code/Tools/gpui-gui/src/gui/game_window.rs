use crate::gui::source_catalog::GuiPortRecord;
use crate::model::{GadgetWindowStyle, LegacyRect, WindowStatus};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GameWindow.cpp",
    "crate::gui::game_window",
    "Game Window",
    "Ports the core window node, tree links, text state, status bits, and region math.",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextColors {
    pub enabled: u32,
    pub enabled_border: u32,
    pub disabled: u32,
    pub disabled_border: u32,
    pub hilite: u32,
    pub hilite_border: u32,
    pub ime_composite: u32,
    pub ime_composite_border: u32,
}

impl Default for TextColors {
    fn default() -> Self {
        Self {
            enabled: 0x00ff_ffff,
            enabled_border: 0,
            disabled: 0x0080_8080,
            disabled_border: 0,
            hilite: 0x00ff_e08a,
            hilite_border: 0,
            ime_composite: 0x008d_c0ff,
            ime_composite_border: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GameWindowPort {
    pub id: i32,
    pub title: String,
    pub text: String,
    pub tooltip: String,
    pub rect: LegacyRect,
    pub cursor_x: i32,
    pub cursor_y: i32,
    pub status: WindowStatus,
    pub style: GadgetWindowStyle,
    pub parent: Option<i32>,
    pub owner: Option<i32>,
    pub next: Option<i32>,
    pub prev: Option<i32>,
    pub child: Option<i32>,
    pub children: Vec<i32>,
    pub next_layout: Option<i32>,
    pub prev_layout: Option<i32>,
    pub layout: Option<String>,
    pub hidden: bool,
    pub enabled: bool,
    pub hilited: bool,
    pub draw_offset: (i32, i32),
    pub font_name: Option<String>,
    pub text_colors: TextColors,
    pub user_data: usize,
    pub instance_data: Option<String>,
}

impl GameWindowPort {
    pub fn new(id: i32, title: impl Into<String>, rect: LegacyRect) -> Self {
        Self {
            id,
            title: title.into(),
            text: String::new(),
            tooltip: String::new(),
            rect,
            cursor_x: 0,
            cursor_y: 0,
            status: WindowStatus::empty(),
            style: GadgetWindowStyle::USER_WINDOW,
            parent: None,
            owner: None,
            next: None,
            prev: None,
            child: None,
            children: Vec::new(),
            next_layout: None,
            prev_layout: None,
            layout: None,
            hidden: false,
            enabled: false,
            hilited: false,
            draw_offset: (0, 0),
            font_name: None,
            text_colors: TextColors::default(),
            user_data: 0,
            instance_data: None,
        }
    }

    pub fn normalize_window_region(&mut self) {
        if self.rect.width < 0 {
            self.rect.x += self.rect.width;
            self.rect.width = -self.rect.width;
        }
        if self.rect.height < 0 {
            self.rect.y += self.rect.height;
            self.rect.height = -self.rect.height;
        }
    }

    pub fn set_position(&mut self, x: i32, y: i32) {
        self.rect.x = x;
        self.rect.y = y;
    }

    pub fn position(&self) -> (i32, i32) {
        (self.rect.x, self.rect.y)
    }

    pub fn set_cursor_position(&mut self, x: i32, y: i32) {
        self.cursor_x = x;
        self.cursor_y = y;
    }

    pub fn cursor_position(&self) -> (i32, i32) {
        (self.cursor_x, self.cursor_y)
    }

    pub fn screen_position(&self, windows: &[GameWindowPort]) -> (i32, i32) {
        let mut x = self.rect.x;
        let mut y = self.rect.y;
        let mut parent = self.parent;
        while let Some(parent_id) = parent {
            let Some(window) = windows.iter().find(|window| window.id == parent_id) else {
                break;
            };
            x += window.rect.x;
            y += window.rect.y;
            parent = window.parent;
        }
        (x, y)
    }

    pub fn region(&self) -> LegacyRect {
        self.rect
    }

    pub fn point_in_window(&self, x: i32, y: i32) -> bool {
        let right = self.rect.x + self.rect.width;
        let bottom = self.rect.y + self.rect.height;
        x >= self.rect.x && y >= self.rect.y && x < right && y < bottom
    }

    pub fn set_size(&mut self, width: i32, height: i32) {
        self.rect.width = width;
        self.rect.height = height;
        self.normalize_window_region();
    }

    pub fn size(&self) -> (i32, i32) {
        (self.rect.width, self.rect.height)
    }

    pub fn enable(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            self.status.insert(WindowStatus::ENABLED);
        } else {
            self.status.remove(WindowStatus::ENABLED);
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn hide(&mut self, hidden: bool) {
        self.hidden = hidden;
        if hidden {
            self.status.insert(WindowStatus::HIDDEN);
        } else {
            self.status.remove(WindowStatus::HIDDEN);
        }
    }

    pub fn is_hidden(&self) -> bool {
        self.hidden
    }

    pub fn set_status(&mut self, status: WindowStatus) -> WindowStatus {
        self.status.insert(status);
        self.status
    }

    pub fn clear_status(&mut self, status: WindowStatus) -> WindowStatus {
        self.status.remove(status);
        self.status
    }

    pub fn get_status(&self) -> WindowStatus {
        self.status
    }

    pub fn get_style(&self) -> GadgetWindowStyle {
        self.style
    }

    pub fn set_hilite_state(&mut self, state: bool) {
        self.hilited = state;
    }

    pub fn set_draw_offset(&mut self, x: i32, y: i32) {
        self.draw_offset = (x, y);
    }

    pub fn draw_offset(&self) -> (i32, i32) {
        self.draw_offset
    }

    pub fn set_text(&mut self, new_text: impl Into<String>) {
        self.text = new_text.into();
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn text_length(&self) -> usize {
        self.text.chars().count()
    }

    pub fn set_font(&mut self, font_name: impl Into<String>) {
        self.font_name = Some(font_name.into());
    }

    pub fn set_enabled_text_colors(&mut self, color: u32, border_color: u32) {
        self.text_colors.enabled = color;
        self.text_colors.enabled_border = border_color;
    }

    pub fn set_disabled_text_colors(&mut self, color: u32, border_color: u32) {
        self.text_colors.disabled = color;
        self.text_colors.disabled_border = border_color;
    }

    pub fn set_hilite_text_colors(&mut self, color: u32, border_color: u32) {
        self.text_colors.hilite = color;
        self.text_colors.hilite_border = border_color;
    }

    pub fn set_ime_composite_text_colors(&mut self, color: u32, border_color: u32) {
        self.text_colors.ime_composite = color;
        self.text_colors.ime_composite_border = border_color;
    }

    pub fn set_instance_data(&mut self, data: impl Into<String>) {
        self.instance_data = Some(data.into());
    }

    pub fn set_user_data(&mut self, data: usize) {
        self.user_data = data;
    }

    pub fn set_tooltip(&mut self, tip: impl Into<String>) {
        self.tooltip = tip.into();
    }

    pub fn set_window_id(&mut self, id: i32) {
        self.id = id;
    }

    pub fn set_parent(&mut self, parent: Option<i32>) {
        self.parent = parent;
    }

    pub fn is_child(&self, child_id: i32, windows: &[GameWindowPort]) -> bool {
        if self.children.contains(&child_id) {
            return true;
        }

        self.children.iter().any(|descendant_id| {
            windows
                .iter()
                .find(|window| window.id == *descendant_id)
                .map(|window| window.is_child(child_id, windows))
                .unwrap_or(false)
        })
    }

    pub fn set_owner(&mut self, owner: Option<i32>) {
        self.owner = owner;
    }

    pub fn set_next(&mut self, next: Option<i32>) {
        self.next = next;
    }

    pub fn set_prev(&mut self, prev: Option<i32>) {
        self.prev = prev;
    }

    pub fn set_next_in_layout(&mut self, next: Option<i32>) {
        self.next_layout = next;
    }

    pub fn set_prev_in_layout(&mut self, prev: Option<i32>) {
        self.prev_layout = prev;
    }

    pub fn set_layout(&mut self, layout: Option<String>) {
        self.layout = layout;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_region_flips_negative_dimensions() {
        let mut window = GameWindowPort::new(
            10,
            "Test",
            LegacyRect {
                x: 100,
                y: 80,
                width: -20,
                height: -10,
            },
        );

        window.normalize_window_region();

        assert_eq!(window.rect.x, 80);
        assert_eq!(window.rect.y, 70);
        assert_eq!(window.rect.width, 20);
        assert_eq!(window.rect.height, 10);
    }

    #[test]
    fn screen_position_accumulates_parent_offsets() {
        let mut parent = GameWindowPort::new(
            1,
            "Parent",
            LegacyRect {
                x: 32,
                y: 48,
                width: 300,
                height: 200,
            },
        );
        let mut child = GameWindowPort::new(
            2,
            "Child",
            LegacyRect {
                x: 10,
                y: 16,
                width: 100,
                height: 50,
            },
        );
        child.set_parent(Some(parent.id));
        parent.children.push(child.id);

        assert_eq!(child.screen_position(&[parent, child.clone()]), (42, 64));
    }
}
