//! UI Widget Components
//!
//! Basic UI widgets like buttons, text, panels, etc.

use super::{Interactive, KeyCode, MouseButton, Renderable, UIRenderContext};

/// Generic UI widget trait
pub trait UIWidget: Renderable + Interactive {
    fn update(&mut self, delta_time: f32);
    fn set_enabled(&mut self, enabled: bool);
    fn is_enabled(&self) -> bool;
}

/// Button widget
pub struct Button {
    pub text: String,
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub enabled: bool,
    pub visible: bool,
    pub hovered: bool,
}

impl Button {
    pub fn new(text: &str, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            text: text.to_string(),
            position: (x, y),
            size: (width, height),
            enabled: true,
            visible: true,
            hovered: false,
        }
    }
}

impl UIWidget for Button {
    fn update(&mut self, _delta_time: f32) {}
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Interactive for Button {
    fn handle_mouse_move(&mut self, _x: i32, _y: i32) -> bool {
        false
    }
    fn handle_mouse_click(&mut self, _x: i32, _y: i32, _button: MouseButton) -> bool {
        false
    }
    fn handle_key_press(&mut self, _key: KeyCode) -> bool {
        false
    }
    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
}

impl Renderable for Button {
    fn render(&self, _context: &mut UIRenderContext) {}
    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (self.position.0, self.position.1, self.size.0, self.size.1)
    }
    fn is_visible(&self) -> bool {
        self.visible
    }
}

/// Text widget
pub struct Text {
    pub content: String,
    pub position: (i32, i32),
    pub visible: bool,
}

impl Text {
    pub fn new(content: &str, x: i32, y: i32) -> Self {
        Self {
            content: content.to_string(),
            position: (x, y),
            visible: true,
        }
    }
}

impl UIWidget for Text {
    fn update(&mut self, _delta_time: f32) {}
    fn set_enabled(&mut self, _enabled: bool) {}
    fn is_enabled(&self) -> bool {
        true
    }
}

impl Interactive for Text {
    fn handle_mouse_move(&mut self, _x: i32, _y: i32) -> bool {
        false
    }
    fn handle_mouse_click(&mut self, _x: i32, _y: i32, _button: MouseButton) -> bool {
        false
    }
    fn handle_key_press(&mut self, _key: KeyCode) -> bool {
        false
    }
    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
}

impl Renderable for Text {
    fn render(&self, _context: &mut UIRenderContext) {}
    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (self.position.0, self.position.1, 100, 20)
    }
    fn is_visible(&self) -> bool {
        self.visible
    }
}

/// Panel widget
pub struct Panel {
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub visible: bool,
}

impl Panel {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            position: (x, y),
            size: (width, height),
            visible: true,
        }
    }
}

impl UIWidget for Panel {
    fn update(&mut self, _delta_time: f32) {}
    fn set_enabled(&mut self, _enabled: bool) {}
    fn is_enabled(&self) -> bool {
        true
    }
}

impl Interactive for Panel {
    fn handle_mouse_move(&mut self, _x: i32, _y: i32) -> bool {
        false
    }
    fn handle_mouse_click(&mut self, _x: i32, _y: i32, _button: MouseButton) -> bool {
        false
    }
    fn handle_key_press(&mut self, _key: KeyCode) -> bool {
        false
    }
    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
}

impl Renderable for Panel {
    fn render(&self, _context: &mut UIRenderContext) {}
    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (self.position.0, self.position.1, self.size.0, self.size.1)
    }
    fn is_visible(&self) -> bool {
        self.visible
    }
}

/// Progress bar widget
pub struct ProgressBar {
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub progress: f32, // 0.0 to 1.0
    pub visible: bool,
}

impl ProgressBar {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            position: (x, y),
            size: (width, height),
            progress: 0.0,
            visible: true,
        }
    }

    pub fn set_progress(&mut self, progress: f32) {
        self.progress = progress.clamp(0.0, 1.0);
    }
}

impl UIWidget for ProgressBar {
    fn update(&mut self, _delta_time: f32) {}
    fn set_enabled(&mut self, _enabled: bool) {}
    fn is_enabled(&self) -> bool {
        true
    }
}

impl Interactive for ProgressBar {
    fn handle_mouse_move(&mut self, _x: i32, _y: i32) -> bool {
        false
    }
    fn handle_mouse_click(&mut self, _x: i32, _y: i32, _button: MouseButton) -> bool {
        false
    }
    fn handle_key_press(&mut self, _key: KeyCode) -> bool {
        false
    }
    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
}

impl Renderable for ProgressBar {
    fn render(&self, _context: &mut UIRenderContext) {}
    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (self.position.0, self.position.1, self.size.0, self.size.1)
    }
    fn is_visible(&self) -> bool {
        self.visible
    }
}

/// Slider widget
pub struct Slider {
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub value: f32, // 0.0 to 1.0
    pub visible: bool,
    pub enabled: bool,
}

impl Slider {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            position: (x, y),
            size: (width, height),
            value: 0.5,
            visible: true,
            enabled: true,
        }
    }

    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(0.0, 1.0);
    }

    pub fn get_value(&self) -> f32 {
        self.value
    }
}

impl UIWidget for Slider {
    fn update(&mut self, _delta_time: f32) {}
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Interactive for Slider {
    fn handle_mouse_move(&mut self, _x: i32, _y: i32) -> bool {
        false
    }
    fn handle_mouse_click(&mut self, _x: i32, _y: i32, _button: MouseButton) -> bool {
        false
    }
    fn handle_key_press(&mut self, _key: KeyCode) -> bool {
        false
    }
    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
}

impl Renderable for Slider {
    fn render(&self, _context: &mut UIRenderContext) {}
    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (self.position.0, self.position.1, self.size.0, self.size.1)
    }
    fn is_visible(&self) -> bool {
        self.visible
    }
}
