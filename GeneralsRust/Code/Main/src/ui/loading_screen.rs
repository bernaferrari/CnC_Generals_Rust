//! Loading Screen
//!
//! This module implements the loading screen with progress bar
//! shown while loading maps and resources.

use super::{Interactive, KeyCode, MouseButton, Renderable, UIRenderContext};
use crate::localization;

/// Loading Screen implementation
pub struct LoadingScreen {
    /// Current loading progress (0.0 to 1.0)
    progress: f32,
    /// Current loading phase description
    current_phase: String,
    /// Map being loaded
    map_name: String,
    /// Screen dimensions
    screen_size: (u32, u32),
    /// Animation counter for spinner
    spinner_frame: u32,
}

impl Default for LoadingScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadingScreen {
    fn text(key: &str, fallback: &str) -> String {
        localization::localize(key, fallback)
    }

    pub fn new() -> Self {
        Self {
            progress: 0.0,
            current_phase: String::new(),
            map_name: String::new(),
            screen_size: (1024, 768),
            spinner_frame: 0,
        }
    }

    pub fn initialize(&mut self, map_name: String) -> Result<(), Box<dyn std::error::Error>> {
        self.progress = 0.0;
        self.map_name = map_name;
        self.current_phase = Self::text("loading.phase_init", "Initializing...");
        self.spinner_frame = 0;
        Ok(())
    }

    pub fn update(&mut self, delta_time: f32) -> Result<(), Box<dyn std::error::Error>> {
        // Update spinner animation
        self.spinner_frame = (self.spinner_frame + 1) % 8;

        // Simulate progress (in real implementation, this would be driven by actual loading)
        if self.progress < 1.0 {
            self.progress += delta_time * 0.3; // 30% per second
            self.progress = self.progress.min(1.0);

            // Update phase based on progress
            self.current_phase = if self.progress < 0.2 {
                Self::text("loading.phase_map", "Loading map data...")
            } else if self.progress < 0.4 {
                Self::text("loading.phase_textures", "Loading textures...")
            } else if self.progress < 0.6 {
                Self::text("loading.phase_models", "Loading models...")
            } else if self.progress < 0.8 {
                Self::text("loading.phase_audio", "Loading audio...")
            } else {
                Self::text("loading.phase_finalize", "Finalizing...")
            };
        }

        Ok(())
    }

    pub fn set_progress(&mut self, progress: f32, phase: String) {
        self.progress = progress.clamp(0.0, 1.0);
        self.current_phase = phase;
    }

    pub fn is_complete(&self) -> bool {
        self.progress >= 1.0
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);
    }

    fn get_spinner_char(&self) -> char {
        let chars = ['|', '/', '-', '\\', '|', '/', '-', '\\'];
        chars[self.spinner_frame as usize % chars.len()]
    }

    fn render_progress_bar(&self, width: usize) -> String {
        let filled = (width as f32 * self.progress) as usize;
        let empty = width - filled;
        format!(
            "[{}{}] {:.0}%",
            "=".repeat(filled),
            " ".repeat(empty),
            self.progress * 100.0
        )
    }
}

impl Interactive for LoadingScreen {
    fn handle_mouse_move(&mut self, _x: i32, _y: i32) -> bool {
        false
    }

    fn handle_mouse_click(&mut self, _x: i32, _y: i32, _button: MouseButton) -> bool {
        false
    }

    fn handle_key_press(&mut self, _key: KeyCode) -> bool {
        false // Loading screen doesn't respond to input
    }

    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
}

impl Renderable for LoadingScreen {
    fn render(&self, _context: &mut UIRenderContext) {
        println!("{}", Self::text("loading.log.header", "=== LOADING ==="));
        println!();
        println!(
            "{} {}",
            Self::text("loading.map_label", "Map:"),
            self.map_name
        );
        println!();
        println!("  {}", self.current_phase);
        println!();
        println!("  {}", self.render_progress_bar(40));
        println!();

        if !self.is_complete() {
            println!(
                "  {} {}",
                Self::text("loading.please_wait", "Please wait"),
                self.get_spinner_char()
            );
        } else {
            println!("  {}", Self::text("loading.complete", "Loading complete!"));
        }
    }

    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (0, 0, self.screen_size.0, self.screen_size.1)
    }

    fn is_visible(&self) -> bool {
        true
    }
}
