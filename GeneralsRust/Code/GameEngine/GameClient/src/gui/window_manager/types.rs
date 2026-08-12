//! Tab navigation, capture flags, and modal window stack nodes.

use std::cell::RefCell;
use std::rc::Rc;

use crate::gui::game_window::GameWindow;

/// Tab navigation direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabDirection {
    Next,
    Previous,
}

/// Capture flags for input capture
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CaptureFlags: u32 {
        const MOUSE = 0x00000001;
        const KEYBOARD = 0x00000002;
        const ALL = 0xFFFFFFFF;
    }
}

/// Modal window wrapper
#[derive(Debug)]
pub struct ModalWindow {
    pub window: Rc<RefCell<GameWindow>>,
    pub next: Option<Box<ModalWindow>>,
}

impl ModalWindow {
    pub fn new(window: Rc<RefCell<GameWindow>>) -> Self {
        Self { window, next: None }
    }
}

impl Clone for ModalWindow {
    fn clone(&self) -> Self {
        Self {
            window: Rc::clone(&self.window),
            next: self.next.as_ref().map(|next| Box::new((**next).clone())),
        }
    }
}
