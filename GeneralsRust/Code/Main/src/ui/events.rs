//! UI Event System
//!
//! This module defines input events and event handling for the UI system.

use super::{KeyCode, MouseButton};

/// Input events for UI components
#[derive(Debug, Clone)]
pub enum InputEvent {
    Mouse(MouseEvent),
    Keyboard(KeyEvent),
    MouseMove { x: i32, y: i32 },
    MouseClick { x: i32, y: i32, button: MouseButton },
    KeyPress { key: KeyCode },
    KeyRelease { key: KeyCode },
    TextInput { text: String },
    WindowResized { width: u32, height: u32 },
    WindowResize { width: u32, height: u32 },
    WindowFocusChanged { focused: bool },
}

/// Keyboard event data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: KeyCode,
    pub pressed: bool,
    pub repeat: bool,
}

impl KeyEvent {
    pub fn new(key: KeyCode, pressed: bool) -> Self {
        Self {
            key,
            pressed,
            repeat: false,
        }
    }

    pub fn with_repeat(key: KeyCode, pressed: bool, repeat: bool) -> Self {
        Self {
            key,
            pressed,
            repeat,
        }
    }
}

/// Mouse event data
#[derive(Debug, Clone)]
pub enum MouseEvent {
    Move {
        x: f32,
        y: f32,
    },
    ButtonDown {
        button: winit::event::MouseButton,
        x: f32,
        y: f32,
    },
    ButtonUp {
        button: winit::event::MouseButton,
        x: f32,
        y: f32,
    },
    Scroll {
        delta: f32,
    },
}

impl MouseEvent {
    pub fn move_event(x: f32, y: f32) -> Self {
        Self::Move { x, y }
    }

    pub fn button_down_event(x: f32, y: f32, button: winit::event::MouseButton) -> Self {
        Self::ButtonDown { button, x, y }
    }

    pub fn button_up_event(x: f32, y: f32, button: winit::event::MouseButton) -> Self {
        Self::ButtonUp { button, x, y }
    }

    pub fn scroll_event(delta: f32) -> Self {
        Self::Scroll { delta }
    }
}

/// Trait for handling UI events
pub trait UIEventHandler {
    fn handle_input_event(&mut self, event: &InputEvent) -> bool;
    fn handle_mouse_event(&mut self, event: &MouseEvent) -> bool;
    fn handle_key_event(&mut self, event: &KeyEvent) -> bool;
}

/// Default implementation for event handlers
impl<T> UIEventHandler for T
where
    T: crate::ui::Interactive,
{
    fn handle_input_event(&mut self, event: &InputEvent) -> bool {
        match event {
            InputEvent::MouseMove { x, y } => self.handle_mouse_move(*x, *y),
            InputEvent::MouseClick { x, y, button } => self.handle_mouse_click(*x, *y, *button),
            InputEvent::KeyPress { key } => self.handle_key_press(*key),
            InputEvent::TextInput { text } => self.handle_text_input(text),
            _ => false,
        }
    }

    fn handle_mouse_event(&mut self, event: &MouseEvent) -> bool {
        match event {
            MouseEvent::Move { x, y } => self.handle_mouse_move(*x as i32, *y as i32),
            MouseEvent::ButtonDown {
                button: _button,
                x,
                y,
            } => {
                // Convert winit::event::MouseButton to UI MouseButton if needed
                self.handle_mouse_move(*x as i32, *y as i32) // For now, just handle movement
            }
            MouseEvent::ButtonUp {
                button: _button,
                x: _x,
                y: _y,
            } => {
                false // Mouse release events not handled by default
            }
            MouseEvent::Scroll { delta: _delta } => {
                false // Scroll events not handled by default
            }
        }
    }

    fn handle_key_event(&mut self, event: &KeyEvent) -> bool {
        if event.pressed {
            self.handle_key_press(event.key)
        } else {
            false // Key release events not handled by default
        }
    }
}
