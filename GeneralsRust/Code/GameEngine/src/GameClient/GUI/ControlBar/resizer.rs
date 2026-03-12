// FILE: resizer.rs
// Port of ControlBarResizer from C++
// Original: ControlBarResizer.h and ControlBarResizer.cpp

use super::scheme::ICoord2D;

/// Resizer Window
/// Contains window sizing data for different control bar configurations
#[derive(Clone, Debug)]
pub struct ResizerWindow {
    /// Window name
    pub name: String,

    /// Default size
    pub default_size: ICoord2D,

    /// Default position
    pub default_pos: ICoord2D,

    /// Alternative size (for compact mode)
    pub alt_size: ICoord2D,

    /// Alternative position (for compact mode)
    pub alt_pos: ICoord2D,
}

impl ResizerWindow {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            default_size: ICoord2D { x: 0, y: 0 },
            default_pos: ICoord2D { x: 0, y: 0 },
            alt_size: ICoord2D { x: 0, y: 0 },
            alt_pos: ICoord2D { x: 0, y: 0 },
        }
    }
}

impl Default for ResizerWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// Control Bar Resizer
/// Manages window resizing for different control bar stages
pub struct ControlBarResizer {
    /// List of resizable windows
    resizer_windows: Vec<ResizerWindow>,
}

impl ControlBarResizer {
    pub fn new() -> Self {
        Self {
            resizer_windows: Vec::new(),
        }
    }

    pub fn init(&mut self) {
        // Load resizer data from INI
    }

    /// Find a resizer window by name
    pub fn find_resizer_window(&self, name: &str) -> Option<&ResizerWindow> {
        self.resizer_windows.iter().find(|w| w.name == name)
    }

    /// Find a resizer window by name (mutable)
    pub fn find_resizer_window_mut(&mut self, name: &str) -> Option<&mut ResizerWindow> {
        self.resizer_windows.iter_mut().find(|w| w.name == name)
    }

    /// Create a new resizer window
    pub fn new_resizer_window(&mut self, name: String) -> &mut ResizerWindow {
        let window = ResizerWindow {
            name: name.clone(),
            ..Default::default()
        };
        self.resizer_windows.push(window);
        self.resizer_windows.last_mut().unwrap()
    }

    /// Size windows to default dimensions
    pub fn size_windows_default(&self) {
        for window in &self.resizer_windows {
            // Apply default size and position to actual game window
            // This would interact with the window manager
        }
    }

    /// Size windows to alternative (compact) dimensions
    pub fn size_windows_alt(&self) {
        for window in &self.resizer_windows {
            // Apply alt size and position to actual game window
            // This would interact with the window manager
        }
    }
}

impl Default for ControlBarResizer {
    fn default() -> Self {
        Self::new()
    }
}
