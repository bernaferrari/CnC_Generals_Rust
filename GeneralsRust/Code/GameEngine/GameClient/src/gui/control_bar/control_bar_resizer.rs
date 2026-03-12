//! Control bar resizer state and policy.
//!
//! Ported from `ControlBarResizer.cpp`.

use super::control_bar::ControlBarResizer;
use std::sync::RwLock;

/// Mirrors the C++ `ResizerWindow` data blob.
#[derive(Debug, Clone)]
pub struct ResizerWindow {
    pub name: String,
    pub default_pos: (i32, i32),
    pub default_size: (u32, u32),
    pub alt_pos: (i32, i32),
    pub alt_size: (u32, u32),
}

impl ResizerWindow {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            default_pos: (0, 0),
            default_size: (0, 0),
            alt_pos: (0, 0),
            alt_size: (0, 0),
        }
    }
}

/// Runtime control-bar resizer.
///
/// The original C++ implementation applies positions directly to `GameWindow`s.
/// The Rust UI stack is transitioning to layout-driven sizing, so we persist the same authored
/// data here and provide deterministic scaling calculations.
#[derive(Debug)]
pub struct IniControlBarResizer {
    windows: RwLock<Vec<ResizerWindow>>,
    base_resolution: (u32, u32),
}

impl Default for IniControlBarResizer {
    fn default() -> Self {
        Self {
            windows: RwLock::new(Vec::new()),
            base_resolution: (800, 600),
        }
    }
}

impl IniControlBarResizer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_window(&self, window: ResizerWindow) {
        if let Ok(mut windows) = self.windows.write() {
            windows.push(window);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut windows) = self.windows.write() {
            windows.clear();
        }
    }

    pub fn window_count(&self) -> usize {
        self.windows.read().map(|w| w.len()).unwrap_or(0)
    }

    pub fn set_base_resolution(&mut self, width: u32, height: u32) {
        self.base_resolution = (width.max(1), height.max(1));
    }
}

impl ControlBarResizer for IniControlBarResizer {
    fn resize(&self, width: u32, height: u32) -> Result<(), Box<dyn std::error::Error>> {
        let (base_w, base_h) = self.base_resolution;
        let scale_x = width as f32 / base_w as f32;
        let scale_y = height as f32 / base_h as f32;

        if let Ok(windows) = self.windows.read() {
            log::trace!(
                "ControlBarResizer resize {} windows to {}x{} (scale {:.3}, {:.3})",
                windows.len(),
                width,
                height,
                scale_x,
                scale_y
            );
        }

        Ok(())
    }

    fn get_optimal_size(&self) -> (u32, u32) {
        self.base_resolution
    }
}
