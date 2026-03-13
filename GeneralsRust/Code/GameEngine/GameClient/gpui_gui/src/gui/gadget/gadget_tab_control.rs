use gpui::{div, prelude::*, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetTabControl.cpp",
    "crate::gui::gadget::gadget_tab_control",
    "Gadget Tab Control",
    "Ports tab selection and pane switching across grouped window content.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Tab Control",
    "Tabs that switch visible panes.",
    "Activate tabs and change pane focus.",
    GadgetKind::TabControl,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabEdgePort {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabOrientationPort {
    TopLeft,
    Center,
    BottomRight,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TabRegionPort {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabControlState {
    pub active_tab: usize,
    pub tab_count: usize,
    pub disabled: Vec<bool>,
    pub pane_border: i32,
    pub tab_width: i32,
    pub tab_height: i32,
    pub edge: TabEdgePort,
    pub orientation: TabOrientationPort,
    pub region: TabRegionPort,
}

impl Default for TabControlState {
    fn default() -> Self {
        Self {
            active_tab: 0,
            tab_count: 3,
            disabled: vec![false, false, false],
            pane_border: 4,
            tab_width: 96,
            tab_height: 28,
            edge: TabEdgePort::Top,
            orientation: TabOrientationPort::TopLeft,
            region: TabRegionPort {
                left: 4,
                right: 292,
                top: 4,
                bottom: 32,
            },
        }
    }
}

impl TabControlState {
    pub fn resize(&mut self, width: i32, height: i32) {
        let mut horz_offset = 0;
        let mut vert_offset = 0;
        if matches!(self.edge, TabEdgePort::Top | TabEdgePort::Bottom) {
            let extra = width - (2 * self.pane_border) - (self.tab_count as i32 * self.tab_width);
            horz_offset = match self.orientation {
                TabOrientationPort::Center => extra / 2,
                TabOrientationPort::BottomRight => extra,
                TabOrientationPort::TopLeft => 0,
            };
        } else {
            let extra = height - (2 * self.pane_border) - (self.tab_count as i32 * self.tab_height);
            vert_offset = match self.orientation {
                TabOrientationPort::Center => extra / 2,
                TabOrientationPort::BottomRight => extra,
                TabOrientationPort::TopLeft => 0,
            };
        }

        self.region = match self.edge {
            TabEdgePort::Top => TabRegionPort {
                left: self.pane_border + horz_offset,
                right: self.pane_border + horz_offset + (self.tab_width * self.tab_count as i32),
                top: self.pane_border,
                bottom: self.pane_border + self.tab_height,
            },
            TabEdgePort::Bottom => TabRegionPort {
                left: self.pane_border + horz_offset,
                right: self.pane_border + horz_offset + (self.tab_width * self.tab_count as i32),
                top: height - self.pane_border - self.tab_height,
                bottom: height - self.pane_border,
            },
            TabEdgePort::Right => TabRegionPort {
                left: width - self.pane_border - self.tab_width,
                right: width - self.pane_border,
                top: self.pane_border + vert_offset,
                bottom: self.pane_border + vert_offset + (self.tab_height * self.tab_count as i32),
            },
            TabEdgePort::Left => TabRegionPort {
                left: self.pane_border,
                right: self.pane_border + self.tab_width,
                top: self.pane_border + vert_offset,
                bottom: self.pane_border + vert_offset + (self.tab_height * self.tab_count as i32),
            },
        };
    }

    pub fn click(&mut self, mouse_x: i32, mouse_y: i32) -> Option<usize> {
        if mouse_x < self.region.left
            || mouse_x > self.region.right
            || mouse_y < self.region.top
            || mouse_y > self.region.bottom
        {
            return None;
        }

        let distance_in = if matches!(self.edge, TabEdgePort::Left | TabEdgePort::Right) {
            mouse_y - self.region.top
        } else {
            mouse_x - self.region.left
        };
        let tab_size = if matches!(self.edge, TabEdgePort::Left | TabEdgePort::Right) {
            self.tab_height
        } else {
            self.tab_width
        };
        let tab = (distance_in / tab_size) as usize;
        if tab >= self.tab_count || self.disabled.get(tab).copied().unwrap_or(false) {
            return None;
        }
        if tab != self.active_tab {
            self.active_tab = tab;
            Some(tab)
        } else {
            None
        }
    }
}

pub fn render_demo(labels: &[&str], active: &str) -> AnyElement {
    div()
        .flex()
        .gap_1()
        .children(labels.iter().map(|label| {
            div()
                .px_3()
                .py_1()
                .rounded_t_md()
                .border_1()
                .border_color(rgb(0x22303f))
                .bg(if *label == active {
                    rgb(0x18232f)
                } else {
                    rgb(0x101720)
                })
                .child((*label).to_string())
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicking_disabled_tab_is_ignored() {
        let mut state = TabControlState {
            disabled: vec![false, true, false],
            ..Default::default()
        };
        state.resize(320, 200);
        assert_eq!(state.click(110, 10), None);
        assert_eq!(state.active_tab, 0);
    }
}
