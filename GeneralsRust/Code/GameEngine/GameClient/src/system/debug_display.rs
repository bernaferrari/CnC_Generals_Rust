//! Debug display base class for on-screen diagnostics.

use crate::gui::game_window::Color;
use log::debug;
use std::fmt;
use std::sync::Arc;

const WHITE: Color = 0xFFFFFFFF;
const BLACK: Color = 0x000000FF;
const W3D_DEBUG_TEXT_Y_OFFSET: i32 = 13;

pub trait DebugTextSink: Send + Sync {
    fn draw_text(&self, x: i32, y: i32, text: &str, color: Color, drop_color: Color);
}

/// DebugDisplay mirrors the C++ DebugDisplay utility used by debug HUDs.
pub struct DebugDisplay {
    width: i32,
    height: i32,
    x_pos: i32,
    y_pos: i32,
    right_margin: i32,
    left_margin: i32,
    text_color: Color,
    font_width: i32,
    font_height: i32,
    has_font: bool,
    display_string_ready: bool,
    sink: Option<Arc<dyn DebugTextSink>>,
}

impl DebugDisplay {
    pub fn new() -> Self {
        let mut display = Self {
            width: 0,
            height: 0,
            x_pos: 0,
            y_pos: 0,
            right_margin: 0,
            left_margin: 0,
            text_color: WHITE,
            font_width: 0,
            font_height: 0,
            has_font: false,
            display_string_ready: false,
            sink: None,
        };
        display.reset();
        display
    }

    pub fn with_sink(sink: Arc<dyn DebugTextSink>) -> Self {
        let mut display = Self::new();
        display.set_sink(Some(sink));
        display
    }

    pub fn set_sink(&mut self, sink: Option<Arc<dyn DebugTextSink>>) {
        self.sink = sink;
    }

    pub fn init(&mut self) {
        self.display_string_ready = true;
    }

    pub fn set_font_available(&mut self, has_font: bool) {
        self.has_font = has_font;
    }

    pub fn set_font_width(&mut self, width: i32) {
        self.font_width = width;
    }

    pub fn set_font_height(&mut self, height: i32) {
        self.font_height = height;
    }

    pub fn reset(&mut self) {
        self.set_cursor_pos(0, 0);
        self.set_text_color(WHITE);
        self.set_right_margin(0);
        self.set_left_margin(self.width);
    }

    pub fn set_dimensions(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
    }

    pub fn set_cursor_pos(&mut self, x: i32, y: i32) {
        self.x_pos = x;
        self.y_pos = y;
    }

    pub fn get_cursor_x(&self) -> i32 {
        self.x_pos
    }

    pub fn get_cursor_y(&self) -> i32 {
        self.y_pos
    }

    pub fn get_width(&self) -> i32 {
        self.width
    }

    pub fn get_height(&self) -> i32 {
        self.height
    }

    pub fn set_text_color(&mut self, color: Color) {
        self.text_color = color;
    }

    pub fn set_right_margin(&mut self, right_pos: i32) {
        self.right_margin = right_pos;
    }

    pub fn set_left_margin(&mut self, left_pos: i32) {
        self.left_margin = left_pos;
    }

    pub fn printf(&mut self, args: fmt::Arguments<'_>) {
        let text = format!("{args}");
        self.write_text(&text);
    }

    pub fn write_text(&mut self, text: &str) {
        let mut line_start = 0usize;
        let mut line_len = 0usize;

        for (idx, ch) in text.char_indices() {
            match ch {
                '\n' => {
                    if line_len > 0 {
                        let line = &text[line_start..idx];
                        self.draw_line(line);
                        line_len = 0;
                    }
                    line_start = idx + 1;
                    self.y_pos += 1;
                    self.x_pos = 0;
                }
                _ => {
                    line_len += 1;
                }
            }
        }

        if line_len > 0 {
            let line = &text[line_start..];
            self.draw_line(line);
            self.x_pos += line_len as i32;
        }
    }

    fn draw_line(&self, text: &str) {
        if !self.has_font || !self.display_string_ready {
            return;
        }

        let x = self.right_margin + self.x_pos;
        let screen_x = x * self.font_width;
        let screen_y = W3D_DEBUG_TEXT_Y_OFFSET + self.y_pos * self.font_height;
        if let Some(sink) = &self.sink {
            sink.draw_text(screen_x, screen_y, text, WHITE, BLACK);
        } else {
            debug!("DebugDisplay [{}:{}] {}", screen_x, screen_y, text);
        }
    }
}

impl Default for DebugDisplay {
    fn default() -> Self {
        Self::new()
    }
}

#[macro_export]
macro_rules! debug_display_printf {
    ($display:expr, $($arg:tt)*) => {{
        $display.printf(format_args!($($arg)*));
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct CaptureSink {
        draws: Mutex<Vec<(i32, i32, String, Color, Color)>>,
    }

    impl DebugTextSink for CaptureSink {
        fn draw_text(&self, x: i32, y: i32, text: &str, color: Color, drop_color: Color) {
            self.draws
                .lock()
                .unwrap()
                .push((x, y, text.to_string(), color, drop_color));
        }
    }

    #[test]
    fn w3d_debug_display_requires_font_and_display_string_like_cpp() {
        let sink = Arc::new(CaptureSink::default());
        let mut display = DebugDisplay::with_sink(sink.clone());
        display.set_font_width(8);
        display.set_font_height(12);

        display.write_text("hidden");
        assert!(sink.draws.lock().unwrap().is_empty());

        display.init();
        display.write_text("still hidden");
        assert!(sink.draws.lock().unwrap().is_empty());

        display.set_font_available(true);
        display.write_text("shown");
        assert_eq!(sink.draws.lock().unwrap().len(), 1);
    }

    #[test]
    fn w3d_debug_display_scales_character_coords_and_uses_cpp_colors() {
        let sink = Arc::new(CaptureSink::default());
        let mut display = DebugDisplay::with_sink(sink.clone());
        display.init();
        display.set_font_available(true);
        display.set_font_width(9);
        display.set_font_height(14);
        display.set_text_color(0xAA00AAFF);
        display.set_right_margin(2);
        display.set_cursor_pos(3, 4);

        display.write_text("alpha\nbeta");

        assert_eq!(
            sink.draws.lock().unwrap().as_slice(),
            &[
                (45, 69, "alpha".to_string(), WHITE, BLACK),
                (18, 83, "beta".to_string(), WHITE, BLACK),
            ]
        );
        assert_eq!(display.get_cursor_x(), 4);
        assert_eq!(display.get_cursor_y(), 5);
    }
}
