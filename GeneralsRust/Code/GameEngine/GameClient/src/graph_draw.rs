//! Graph draw helper (ported from `GraphDraw.cpp`).

use crate::gui::display_string::{get_display_string_manager, DisplayStringHandle};
use crate::gui::font::get_font_library;
use crate::gui::ui_globals::with_ui_renderer;
use crate::gui::ui_renderer::UIRect;

const MAX_GRAPH_VALUES: usize = 128;
const BAR_HEIGHT: i32 = 14;
const BAR_SPACE: i32 = 4;

pub struct GraphDraw {
    entries: Vec<(String, f32)>,
    display_strings: Vec<DisplayStringHandle>,
}

impl GraphDraw {
    pub fn new() -> Self {
        let mut manager = get_display_string_manager();
        let mut display_strings = Vec::with_capacity(MAX_GRAPH_VALUES);
        for _ in 0..MAX_GRAPH_VALUES {
            display_strings.push(manager.new_display_string());
        }
        drop(manager);

        if let Ok(font) = get_font_library().get_font_by_name("Courier", 10, false) {
            for handle in &display_strings {
                handle.borrow_mut().set_font(font.clone());
            }
        }

        Self {
            entries: Vec::new(),
            display_strings,
        }
    }

    pub fn add_entry(&mut self, label: impl Into<String>, value: f32) {
        self.entries.push((label.into(), value));
    }

    pub fn render(&mut self) {
        let entries_len = self.entries.len().min(MAX_GRAPH_VALUES);
        if entries_len == 0 {
            return;
        }

        let Some((width, height)) = with_ui_renderer(|renderer| {
            renderer
                .read()
                .map(|renderer| renderer.screen_size())
                .unwrap_or((0, 0))
        }) else {
            return;
        };
        let width = width as i32;
        let height = height as i32;

        let start = (width as f32 * 0.33) as i32;
        let mut bar_width = width - start;
        if bar_width <= 0 {
            return;
        }

        if BAR_HEIGHT * entries_len as i32 >= height {
            bar_width = width / 2;
        }

        for (idx, (label, value)) in self.entries.iter().take(entries_len).enumerate() {
            if let Some(display) = self.display_strings.get(idx) {
                display.borrow_mut().set_text(label.clone());
                display.borrow_mut().draw_with_drop(
                    5,
                    idx as i32 * BAR_HEIGHT,
                    0xFFFFFFFF,
                    0x00000000,
                    1,
                    1,
                );
            }

            let bar_len = (value / 100000.0 * bar_width as f32) as i32;
            let rect = UIRect::new(
                start as f32,
                (idx as i32 * BAR_HEIGHT - (BAR_SPACE / 2)) as f32,
                bar_len.max(0) as f32,
                (BAR_HEIGHT - BAR_SPACE) as f32,
            );
            let _ = with_ui_renderer(|renderer| {
                if let Ok(mut renderer) = renderer.write() {
                    renderer.draw_rect(rect, [0.5, 0.5, 0.5, 0.5], 0.0);
                }
            });
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for GraphDraw {
    fn default() -> Self {
        Self::new()
    }
}
