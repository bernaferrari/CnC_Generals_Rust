//! Quit Confirmation Dialog
//!
//! This module implements the quit confirmation dialog
//! matching the C&C Generals message box system.

use super::{
    layout, utils, ClickSpring, Interactive, KeyCode, MouseButton, Renderable, UIEvent,
    UIRenderContext,
};
use crate::localization;

/// Dialog button types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DialogButton {
    Yes,
    No,
}

struct DialogBtn {
    button_type: DialogButton,
    text: String,
    position: (i32, i32),
    size: (u32, u32),
    hovered: bool,
    click_spring: ClickSpring,
}

impl DialogBtn {
    fn new(button_type: DialogButton, text: String, x: i32, y: i32) -> Self {
        Self {
            button_type,
            text,
            position: (x, y),
            size: (100, 40),
            hovered: false,
            click_spring: ClickSpring::new(),
        }
    }

    fn contains_point(&self, x: i32, y: i32) -> bool {
        utils::point_in_rect(
            (x, y),
            (self.position.0, self.position.1, self.size.0, self.size.1),
        )
    }

    fn update(&mut self, delta_time: f32) {
        self.click_spring.update(delta_time);
    }

    fn trigger_click(&mut self) {
        self.click_spring.trigger();
    }

    fn click_scale(&self) -> f32 {
        self.click_spring.scale()
    }
}

/// Quit Dialog implementation
pub struct QuitDialog {
    /// Message to display
    message: String,
    /// Yes/No buttons
    buttons: Vec<DialogBtn>,
    /// Screen dimensions
    screen_size: (u32, u32),
    /// Dialog result (None = still open, Some(true) = Yes, Some(false) = No)
    result: Option<bool>,
}

impl Default for QuitDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl QuitDialog {
    fn text(key: &str, fallback: &str) -> String {
        localization::localize(key, fallback)
    }

    pub fn new() -> Self {
        Self {
            message: Self::text("quit.message", "Are you sure you want to quit?"),
            buttons: Vec::new(),
            screen_size: (1024, 768),
            result: None,
        }
    }

    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.setup_buttons();
        self.result = None;
        Ok(())
    }

    pub fn update(&mut self, _delta_time: f32) -> Result<(), Box<dyn std::error::Error>> {
        for button in &mut self.buttons {
            button.update(_delta_time);
        }
        Ok(())
    }

    pub fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> Option<UIEvent> {
        if button != MouseButton::Left {
            return None;
        }

        for dialog_btn in &mut self.buttons {
            if dialog_btn.contains_point(x, y) {
                dialog_btn.trigger_click();
                match dialog_btn.button_type {
                    DialogButton::Yes => {
                        self.result = Some(true);
                        return Some(UIEvent::ExitGame);
                    }
                    DialogButton::No => {
                        self.result = Some(false);
                        return None;
                    }
                }
            }
        }

        None
    }

    pub fn get_result(&self) -> Option<bool> {
        self.result
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);
        self.setup_buttons();
    }

    fn setup_buttons(&mut self) {
        self.buttons.clear();

        let center_x = self.screen_size.0 as i32 / 2;
        let center_y = self.screen_size.1 as i32 / 2;

        self.buttons.push(DialogBtn::new(
            DialogButton::Yes,
            Self::text("quit.yes", "Yes"),
            center_x - 120,
            center_y + 40,
        ));

        self.buttons.push(DialogBtn::new(
            DialogButton::No,
            Self::text("quit.no", "No"),
            center_x + 20,
            center_y + 40,
        ));
    }
}

impl Interactive for QuitDialog {
    fn handle_mouse_move(&mut self, x: i32, y: i32) -> bool {
        let mut handled = false;

        for button in &mut self.buttons {
            let was_hovered = button.hovered;
            let is_hovered = button.contains_point(x, y);
            if is_hovered != was_hovered {
                button.hovered = is_hovered;
                handled = true;
            }
        }

        handled
    }

    fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> bool {
        self.handle_mouse_click(x, y, button).is_some()
    }

    fn handle_key_press(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Escape | KeyCode::N => {
                self.result = Some(false);
                true
            }
            KeyCode::Enter | KeyCode::Y => {
                self.result = Some(true);
                true
            }
            _ => false,
        }
    }

    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
}

impl Renderable for QuitDialog {
    fn render(&self, _context: &mut UIRenderContext) {
        println!("{}", Self::text("quit.log.header", "=== QUIT GAME ==="));
        println!();
        println!("  {}", self.message);
        println!();

        for button in &self.buttons {
            let scale = button.click_scale();
            let (_x, _y, _w, _h) = utils::scale_rect_center(
                (
                    button.position.0,
                    button.position.1,
                    button.size.0,
                    button.size.1,
                ),
                scale,
            );
            let state = if button.hovered { " [HOVER]" } else { "" };
            println!("  [{}]{}", button.text, state);
        }

        println!();
        println!(
            "{}",
            Self::text("quit.log.hint", "Y = Yes | N = No | ESC = Cancel")
        );
    }

    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (0, 0, self.screen_size.0, self.screen_size.1)
    }

    fn is_visible(&self) -> bool {
        true
    }
}
