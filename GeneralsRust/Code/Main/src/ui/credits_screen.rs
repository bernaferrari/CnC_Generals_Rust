//! Credits Screen
//!
//! This module implements the credits screen with scrolling text
//! matching the C&C Generals credits from CreditsMenu.cpp.

use super::{Interactive, KeyCode, MouseButton, Renderable, UIRenderContext};
use crate::localization;

/// Credits Screen implementation (from C++ CreditsMenu.cpp)
pub struct CreditsScreen {
    /// Scroll position (0.0 = start, 1.0 = end)
    scroll_position: f32,
    /// Scroll speed (units per second)
    scroll_speed: f32,
    /// Credits text lines
    credits_text: Vec<String>,
    /// Screen dimensions
    screen_size: (u32, u32),
    /// Whether credits have finished scrolling
    finished: bool,
}

impl Default for CreditsScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl CreditsScreen {
    fn text(key: &str, fallback: &str) -> String {
        localization::localize(key, fallback)
    }

    pub fn new() -> Self {
        Self {
            scroll_position: 0.0,
            scroll_speed: 0.1,
            credits_text: Vec::new(),
            screen_size: (1024, 768),
            finished: false,
        }
    }

    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.load_credits_text();
        self.scroll_position = 0.0;
        self.finished = false;
        Ok(())
    }

    pub fn update(&mut self, delta_time: f32) -> Result<(), Box<dyn std::error::Error>> {
        if !self.finished {
            self.scroll_position += self.scroll_speed * delta_time;

            // Credits finish when scrolled past all text
            if self.scroll_position >= 1.0 {
                self.finished = true;
            }
        }
        Ok(())
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn skip_to_end(&mut self) {
        self.finished = true;
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);
    }

    fn load_credits_text(&mut self) {
        self.credits_text = vec![
            Self::text("credits.title", "COMMAND & CONQUER"),
            Self::text("credits.subtitle", "GENERALS ZERO HOUR"),
            "".to_string(),
            "".to_string(),
            Self::text("credits.ea_label", "ELECTRONIC ARTS"),
            "".to_string(),
            Self::text("credits.executive_producer", "Executive Producer"),
            "Mark Skaggs".to_string(),
            "".to_string(),
            Self::text("credits.producer", "Producer"),
            "Greg Black".to_string(),
            "".to_string(),
            Self::text("credits.lead_designer", "Lead Designer"),
            "David Baker".to_string(),
            "".to_string(),
            Self::text("credits.programming", "Programming"),
            "Michael Lightner".to_string(),
            "Julio Jerez".to_string(),
            "Keith Brors".to_string(),
            "".to_string(),
            Self::text("credits.art", "Art Direction"),
            "Craig Fryar".to_string(),
            "".to_string(),
            Self::text("credits.artists", "Artists"),
            "Aaron Cohen".to_string(),
            "Jonathan Wilson".to_string(),
            "".to_string(),
            Self::text("credits.audio", "Audio"),
            "Bill Brown".to_string(),
            "Mikael Sandgren".to_string(),
            "".to_string(),
            Self::text("credits.qa", "Quality Assurance"),
            "EA Pacific QA Team".to_string(),
            "".to_string(),
            Self::text("credits.special_thanks", "Special Thanks"),
            "The C&C Community".to_string(),
            "".to_string(),
            "".to_string(),
            Self::text("credits.rust_port", "RUST PORT"),
            "".to_string(),
            Self::text("credits.rust_team", "Community Contributors"),
            "Open Source Developers".to_string(),
            "".to_string(),
            "".to_string(),
            Self::text("credits.thanks", "THANK YOU FOR PLAYING"),
            "".to_string(),
        ];
    }
}

impl Interactive for CreditsScreen {
    fn handle_mouse_move(&mut self, _x: i32, _y: i32) -> bool {
        false
    }

    fn handle_mouse_click(&mut self, _x: i32, _y: i32, _button: MouseButton) -> bool {
        self.skip_to_end();
        true
    }

    fn handle_key_press(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Escape | KeyCode::Space | KeyCode::Enter => {
                self.skip_to_end();
                true
            }
            _ => false,
        }
    }

    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
}

impl Renderable for CreditsScreen {
    fn render(&self, _context: &mut UIRenderContext) {
        println!("{}", Self::text("credits.log.header", "=== CREDITS ==="));
        println!(
            "{} {:.1}%",
            Self::text("credits.log.scroll", "Scroll:"),
            self.scroll_position * 100.0
        );
        println!();

        // Calculate which lines to show based on scroll position
        let total_lines = self.credits_text.len();
        let visible_lines = 20;
        let start_line = ((total_lines as f32 * self.scroll_position) as usize)
            .saturating_sub(visible_lines / 2);

        for i in start_line..start_line.min(total_lines).min(start_line + visible_lines) {
            if i < self.credits_text.len() {
                println!("  {}", self.credits_text[i]);
            }
        }

        println!();
        if !self.finished {
            println!(
                "{}",
                Self::text("credits.log.press_key", "Press any key to skip...")
            );
        } else {
            println!("{}", Self::text("credits.log.finished", "-- END --"));
        }
    }

    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (0, 0, self.screen_size.0, self.screen_size.1)
    }

    fn is_visible(&self) -> bool {
        true
    }
}
