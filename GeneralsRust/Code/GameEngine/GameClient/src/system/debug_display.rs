//! Debug display base class for on-screen diagnostics.

use crate::gui::game_window::Color;
use log::debug;
use std::fmt;
use std::sync::Arc;

const WHITE: Color = 0xFFFFFFFF;

pub trait DebugTextSink: Send + Sync {
    fn draw_text(&self, x: i32, y: i32, text: &str, color: Color);
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
        let x = self.right_margin + self.x_pos;
        if let Some(sink) = &self.sink {
            sink.draw_text(x, self.y_pos, text, self.text_color);
        } else {
            debug!("DebugDisplay [{}:{}] {}", x, self.y_pos, text);
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
