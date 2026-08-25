//! Optional GPUI chrome host.
//!
//! Mirrors the same [`crate::chrome`] models used by the egui `ToolApp` path.
//! Enabling this module does **not** replace egui; macOS tools continue to
//! `cargo check` / `cargo test` the default egui shell.
//!
//! # Chrome layout (same as egui)
//!
//! ```text
//! +------------------------------------------------------------------+
//! | MenuBar                                                          |
//! +----------+----------------------------------------+--------------+
//! | Left     | Center viewport                        | Right        |
//! | Palette  |                                        | Properties   |
//! +----------+----------------------------------------+--------------+
//! | StatusBar                                                        |
//! +------------------------------------------------------------------+
//! ```

use crate::chrome::{Chrome, ChromeLayout, MenuBar, PaletteTool, StatusBar, ToolPalette};
use gpui::{Div, div, prelude::*, px, rgb};

/// GPUI-facing wrapper around the shared chrome models.
#[derive(Debug, Clone)]
pub struct GpuiChrome {
    inner: Chrome,
}

impl Default for GpuiChrome {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuiChrome {
    pub fn new() -> Self {
        Self {
            inner: Chrome::new(),
        }
    }

    pub fn from_chrome(chrome: Chrome) -> Self {
        Self { inner: chrome }
    }

    pub fn chrome(&self) -> &Chrome {
        &self.inner
    }

    pub fn chrome_mut(&mut self) -> &mut Chrome {
        &mut self.inner
    }

    pub fn menu_bar(&self) -> &MenuBar {
        &self.inner.menu_bar
    }

    pub fn status_bar(&self) -> &StatusBar {
        &self.inner.status_bar
    }

    pub fn status_bar_mut(&mut self) -> &mut StatusBar {
        &mut self.inner.status_bar
    }

    pub fn tool_palette(&self) -> &ToolPalette {
        &self.inner.tool_palette
    }

    pub fn tool_palette_mut(&mut self) -> &mut ToolPalette {
        &mut self.inner.tool_palette
    }

    pub fn layout(&self) -> &ChromeLayout {
        &self.inner.layout
    }

    pub fn select_tool(&mut self, id: &str) -> bool {
        self.inner.tool_palette.select(id)
    }

    pub fn selected_tool_id(&self) -> Option<&str> {
        self.inner.tool_palette.selected_id()
    }

    pub fn set_status_message(&mut self, message: impl Into<String>) {
        self.inner.status_bar.set_message(message);
    }

    pub fn status_message(&self) -> &str {
        self.inner.status_bar.message()
    }

    /// Dock region ids in paint order (top / left / center / right / bottom).
    pub fn region_order() -> &'static [&'static str] {
        ChromeLayout::region_order()
    }

    /// Build a gpui element tree that mirrors the egui chrome docks.
    pub fn render_shell(&self) -> Div {
        render_chrome_shell(&self.inner)
    }
}

/// Map chrome models onto a gpui flex tree (menu / body / status).
pub fn render_chrome_shell(chrome: &Chrome) -> Div {
    div()
        .flex()
        .flex_col()
        .size_full()
        .bg(rgb(0x1a1d23))
        .child(render_menu_bar(&chrome.menu_bar))
        .child(render_body(chrome))
        .child(render_status_bar(&chrome.status_bar))
}

fn render_menu_bar(menu_bar: &MenuBar) -> Div {
    let mut row = div()
        .id("gpui-chrome-menu")
        .flex()
        .flex_row()
        .items_center()
        .gap_4()
        .px(px(8.0))
        .py(px(4.0))
        .bg(rgb(0x252830))
        .text_color(rgb(0xe6e8ee));

    for menu in menu_bar.menus() {
        row = row.child(
            div()
                .id(menu.id.clone())
                .text_sm()
                .child(menu.label.clone()),
        );
    }
    row
}

fn render_body(chrome: &Chrome) -> Div {
    let mut body = div()
        .id("gpui-chrome-body")
        .flex()
        .flex_row()
        .flex_1()
        .min_h(px(1.0));

    if chrome.layout.show_left_palette {
        body = body.child(render_palette(
            &chrome.tool_palette,
            chrome.layout.left_width,
        ));
    }

    body = body.child(
        div()
            .id("gpui-chrome-viewport")
            .flex_1()
            .bg(rgb(0x0f1218))
            .text_color(rgb(0x9aa3b2))
            .p(px(8.0))
            .child("Center viewport"),
    );

    if chrome.layout.show_right_properties {
        body = body.child(
            div()
                .id("gpui-chrome-properties")
                .w(px(chrome.layout.right_width))
                .bg(rgb(0x22262e))
                .text_color(rgb(0xc5cad3))
                .p(px(8.0))
                .child("Properties"),
        );
    }

    body
}

fn render_palette(palette: &ToolPalette, width: f32) -> Div {
    let mut list = div()
        .id("gpui-chrome-palette")
        .w(px(width))
        .bg(rgb(0x22262e))
        .text_color(rgb(0xe6e8ee))
        .p(px(8.0))
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_sm().child("Tools"));

    for tool in palette.tools() {
        list = list.child(render_palette_tool(
            tool,
            palette.selected_id() == Some(tool.id.as_str()),
        ));
    }
    list
}

fn render_palette_tool(tool: &PaletteTool, selected: bool) -> Div {
    let bg = if selected {
        rgb(0x3a5f9e)
    } else {
        rgb(0x22262e)
    };
    div()
        .id(tool.id.clone())
        .bg(bg)
        .px(px(6.0))
        .py(px(2.0))
        .child(tool.name.clone())
}

fn render_status_bar(status: &StatusBar) -> Div {
    let mut line = status.message().to_string();
    if let Some([x, y]) = status.cursor() {
        line.push_str(&format!("  |  Cursor: {x:.0}, {y:.0}"));
    }
    if let Some([x, y]) = status.map_coords() {
        line.push_str(&format!("  |  Map: {x:.1}, {y:.1}"));
    }
    line.push_str(&format!("  |  Zoom: {:.0}%", status.zoom_percent()));

    div()
        .id("gpui-chrome-status")
        .flex()
        .flex_row()
        .items_center()
        .px(px(8.0))
        .py(px(4.0))
        .bg(rgb(0x252830))
        .text_color(rgb(0xc5cad3))
        .text_xs()
        .child(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpui_chrome_mirrors_default_menus_and_selection() {
        let mut chrome = GpuiChrome::new();
        assert!(chrome.menu_bar().has_menu("File"));
        assert!(chrome.menu_bar().has_menu("Edit"));
        assert!(chrome.menu_bar().has_menu("View"));
        assert!(chrome.select_tool("move"));
        assert_eq!(chrome.selected_tool_id(), Some("move"));
        chrome.set_status_message("Ready to edit");
        assert_eq!(chrome.status_message(), "Ready to edit");
        assert_eq!(GpuiChrome::region_order()[1], "left_palette");
    }
}
