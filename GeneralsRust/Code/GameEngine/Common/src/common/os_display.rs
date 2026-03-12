//! OS Display Integration Module
//!
//! Provides platform-specific display management:
//! - Display mode enumeration and switching
//! - Window management (fullscreen, windowed, borderless)
//! - Multi-monitor support
//! - Display settings persistence
//! - Gamma/brightness control

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Display mode representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DisplayMode {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Bits per pixel (color depth)
    pub bits_per_pixel: u32,
    /// Refresh rate in Hz
    pub refresh_rate: u32,
}

impl DisplayMode {
    pub fn new(width: u32, height: u32, bits_per_pixel: u32, refresh_rate: u32) -> Self {
        Self {
            width,
            height,
            bits_per_pixel,
            refresh_rate,
        }
    }

    /// Get aspect ratio as a float
    pub fn aspect_ratio(&self) -> f32 {
        self.width as f32 / self.height as f32
    }

    /// Get total pixel count
    pub fn pixel_count(&self) -> u32 {
        self.width * self.height
    }

    /// Check if this is a standard 16:9 resolution
    pub fn is_16_9(&self) -> bool {
        let ratio = self.aspect_ratio();
        (ratio - 16.0 / 9.0).abs() < 0.01
    }

    /// Check if this is a standard 4:3 resolution
    pub fn is_4_3(&self) -> bool {
        let ratio = self.aspect_ratio();
        (ratio - 4.0 / 3.0).abs() < 0.01
    }

    /// Format as a user-friendly string
    pub fn format_string(&self) -> String {
        format!("{}x{}@{}Hz", self.width, self.height, self.refresh_rate)
    }
}

impl Default for DisplayMode {
    fn default() -> Self {
        Self {
            width: 1024,
            height: 768,
            bits_per_pixel: 32,
            refresh_rate: 60,
        }
    }
}

/// Window mode type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowMode {
    /// Exclusive fullscreen mode
    Fullscreen,
    /// Windowed mode with borders
    Windowed,
    /// Borderless windowed mode (fullscreen window)
    BorderlessWindow,
}

impl Default for WindowMode {
    fn default() -> Self {
        WindowMode::Windowed
    }
}

/// Monitor information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Monitor {
    /// Monitor index
    pub index: usize,
    /// Monitor name
    pub name: String,
    /// Whether this is the primary monitor
    pub is_primary: bool,
    /// Current display mode
    pub current_mode: DisplayMode,
    /// Supported display modes
    pub supported_modes: Vec<DisplayMode>,
    /// Monitor physical position (for multi-monitor setups)
    pub position_x: i32,
    pub position_y: i32,
}

impl Monitor {
    pub fn new(index: usize) -> Self {
        Self {
            index,
            name: format!("Monitor {}", index),
            is_primary: index == 0,
            current_mode: DisplayMode::default(),
            supported_modes: vec![DisplayMode::default()],
            position_x: 0,
            position_y: 0,
        }
    }

    /// Check if a display mode is supported
    pub fn supports_mode(&self, mode: &DisplayMode) -> bool {
        self.supported_modes.contains(mode)
    }

    /// Get all supported resolutions (unique width/height combinations)
    pub fn get_resolutions(&self) -> Vec<(u32, u32)> {
        let mut resolutions: Vec<(u32, u32)> = self
            .supported_modes
            .iter()
            .map(|m| (m.width, m.height))
            .collect();
        resolutions.sort_unstable();
        resolutions.dedup();
        resolutions
    }

    /// Get supported refresh rates for a given resolution
    pub fn get_refresh_rates_for_resolution(&self, width: u32, height: u32) -> Vec<u32> {
        let mut rates: Vec<u32> = self
            .supported_modes
            .iter()
            .filter(|m| m.width == width && m.height == height)
            .map(|m| m.refresh_rate)
            .collect();
        rates.sort_unstable();
        rates.dedup();
        rates
    }
}

/// Display configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    /// Current window mode
    pub window_mode: WindowMode,
    /// Selected monitor index
    pub monitor_index: usize,
    /// Target display mode
    pub target_mode: DisplayMode,
    /// VSync enabled
    pub vsync: bool,
    /// Gamma correction value (1.0 = no correction)
    pub gamma: f32,
    /// Brightness adjustment (1.0 = normal)
    pub brightness: f32,
    /// Contrast adjustment (1.0 = normal)
    pub contrast: f32,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            window_mode: WindowMode::default(),
            monitor_index: 0,
            target_mode: DisplayMode::default(),
            vsync: true,
            gamma: 1.0,
            brightness: 1.0,
            contrast: 1.0,
        }
    }
}

/// Display manager for handling display operations
#[derive(Debug)]
pub struct DisplayManager {
    /// Available monitors
    monitors: Vec<Monitor>,
    /// Current display configuration
    config: DisplayConfig,
    /// Custom display modes added by user
    custom_modes: HashMap<usize, Vec<DisplayMode>>,
}

impl Default for DisplayManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DisplayManager {
    pub fn new() -> Self {
        let mut manager = Self {
            monitors: Vec::new(),
            config: DisplayConfig::default(),
            custom_modes: HashMap::new(),
        };
        manager.detect_monitors();
        manager
    }

    /// Detect available monitors and their capabilities
    pub fn detect_monitors(&mut self) {
        self.monitors.clear();

        // In a real implementation, this would use platform-specific APIs:
        // - Windows: EnumDisplayDevices, EnumDisplaySettings
        // - Linux: X11/Wayland APIs
        // - macOS: NSScreen

        // For now, create a default monitor
        let mut monitor = Monitor::new(0);
        monitor.is_primary = true;
        monitor.name = String::from("Primary Display");

        // Add common display modes
        let common_modes = vec![
            DisplayMode::new(800, 600, 32, 60),
            DisplayMode::new(1024, 768, 32, 60),
            DisplayMode::new(1280, 720, 32, 60),
            DisplayMode::new(1280, 1024, 32, 60),
            DisplayMode::new(1366, 768, 32, 60),
            DisplayMode::new(1600, 900, 32, 60),
            DisplayMode::new(1920, 1080, 32, 60),
            DisplayMode::new(1920, 1200, 32, 60),
            DisplayMode::new(2560, 1440, 32, 60),
            DisplayMode::new(3840, 2160, 32, 60),
        ];
        monitor.supported_modes = common_modes;
        monitor.current_mode = DisplayMode::new(1920, 1080, 32, 60);

        self.monitors.push(monitor);
    }

    /// Get all available monitors
    pub fn get_monitors(&self) -> &[Monitor] {
        &self.monitors
    }

    /// Get primary monitor
    pub fn get_primary_monitor(&self) -> Option<&Monitor> {
        self.monitors.iter().find(|m| m.is_primary)
    }

    /// Get monitor by index
    pub fn get_monitor(&self, index: usize) -> Option<&Monitor> {
        self.monitors.get(index)
    }

    /// Get current display configuration
    pub fn get_config(&self) -> &DisplayConfig {
        &self.config
    }

    /// Set display configuration
    pub fn set_config(&mut self, config: DisplayConfig) -> Result<(), String> {
        // Validate configuration
        if config.monitor_index >= self.monitors.len() {
            return Err(format!("Invalid monitor index: {}", config.monitor_index));
        }

        let monitor = &self.monitors[config.monitor_index];
        if !monitor.supports_mode(&config.target_mode) {
            return Err(format!(
                "Display mode {} not supported on monitor {}",
                config.target_mode.format_string(),
                monitor.name
            ));
        }

        // In a real implementation, this would apply the settings
        self.config = config;
        Ok(())
    }

    /// Set window mode
    pub fn set_window_mode(&mut self, mode: WindowMode) -> Result<(), String> {
        self.config.window_mode = mode;
        // In a real implementation, this would switch the window mode
        Ok(())
    }

    /// Set display resolution
    pub fn set_resolution(&mut self, width: u32, height: u32) -> Result<(), String> {
        let monitor = self
            .monitors
            .get(self.config.monitor_index)
            .ok_or("Invalid monitor index")?;

        // Find a matching mode
        let mode = monitor
            .supported_modes
            .iter()
            .find(|m| m.width == width && m.height == height)
            .ok_or_else(|| format!("Resolution {}x{} not supported", width, height))?;

        self.config.target_mode = *mode;
        Ok(())
    }

    /// Set refresh rate
    pub fn set_refresh_rate(&mut self, refresh_rate: u32) -> Result<(), String> {
        let monitor = self
            .monitors
            .get(self.config.monitor_index)
            .ok_or("Invalid monitor index")?;

        // Find a mode with matching resolution and refresh rate
        let mode = monitor
            .supported_modes
            .iter()
            .find(|m| {
                m.width == self.config.target_mode.width
                    && m.height == self.config.target_mode.height
                    && m.refresh_rate == refresh_rate
            })
            .ok_or_else(|| format!("Refresh rate {} Hz not supported", refresh_rate))?;

        self.config.target_mode = *mode;
        Ok(())
    }

    /// Set gamma correction
    pub fn set_gamma(&mut self, gamma: f32) {
        self.config.gamma = gamma.clamp(0.5, 2.0);
        // In a real implementation, this would apply gamma correction
    }

    /// Set brightness
    pub fn set_brightness(&mut self, brightness: f32) {
        self.config.brightness = brightness.clamp(0.5, 2.0);
        // In a real implementation, this would adjust brightness
    }

    /// Set contrast
    pub fn set_contrast(&mut self, contrast: f32) {
        self.config.contrast = contrast.clamp(0.5, 2.0);
        // In a real implementation, this would adjust contrast
    }

    /// Enable or disable VSync
    pub fn set_vsync(&mut self, enabled: bool) {
        self.config.vsync = enabled;
        // In a real implementation, this would enable/disable VSync
    }

    /// Add a custom display mode
    pub fn add_custom_mode(&mut self, monitor_index: usize, mode: DisplayMode) {
        self.custom_modes
            .entry(monitor_index)
            .or_insert_with(Vec::new)
            .push(mode);
    }

    /// Get all display modes for a monitor (including custom)
    pub fn get_all_modes(&self, monitor_index: usize) -> Vec<DisplayMode> {
        let mut modes = Vec::new();

        if let Some(monitor) = self.monitors.get(monitor_index) {
            modes.extend(monitor.supported_modes.iter().copied());
        }

        if let Some(custom) = self.custom_modes.get(&monitor_index) {
            modes.extend(custom.iter().copied());
        }

        modes.sort_by(|a, b| {
            a.width
                .cmp(&b.width)
                .then(a.height.cmp(&b.height))
                .then(a.refresh_rate.cmp(&b.refresh_rate))
        });
        modes.dedup();
        modes
    }

    /// Generate a display settings report
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("Display Configuration Report\n");
        report.push_str("===========================\n\n");

        report.push_str(&format!("Window Mode: {:?}\n", self.config.window_mode));
        report.push_str(&format!(
            "Resolution: {}\n",
            self.config.target_mode.format_string()
        ));
        report.push_str(&format!("VSync: {}\n", self.config.vsync));
        report.push_str(&format!("Gamma: {:.2}\n", self.config.gamma));
        report.push_str(&format!("Brightness: {:.2}\n", self.config.brightness));
        report.push_str(&format!("Contrast: {:.2}\n\n", self.config.contrast));

        for monitor in &self.monitors {
            report.push_str(&format!("Monitor {}: {}\n", monitor.index, monitor.name));
            report.push_str(&format!(
                "  Primary: {}\n",
                if monitor.is_primary { "Yes" } else { "No" }
            ));
            report.push_str(&format!(
                "  Current: {}\n",
                monitor.current_mode.format_string()
            ));
            report.push_str(&format!(
                "  Position: ({}, {})\n",
                monitor.position_x, monitor.position_y
            ));
            report.push_str(&format!(
                "  Supported Modes: {}\n\n",
                monitor.supported_modes.len()
            ));
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_mode_creation() {
        let mode = DisplayMode::new(1920, 1080, 32, 60);
        assert_eq!(mode.width, 1920);
        assert_eq!(mode.height, 1080);
        assert_eq!(mode.bits_per_pixel, 32);
        assert_eq!(mode.refresh_rate, 60);
    }

    #[test]
    fn test_aspect_ratio() {
        let mode_16_9 = DisplayMode::new(1920, 1080, 32, 60);
        assert!(mode_16_9.is_16_9());
        assert!(!mode_16_9.is_4_3());

        let mode_4_3 = DisplayMode::new(1024, 768, 32, 60);
        assert!(!mode_4_3.is_16_9());
        assert!(mode_4_3.is_4_3());
    }

    #[test]
    fn test_display_manager() {
        let manager = DisplayManager::new();
        assert!(!manager.get_monitors().is_empty());
        assert!(manager.get_primary_monitor().is_some());
    }

    #[test]
    fn test_set_resolution() {
        let mut manager = DisplayManager::new();
        let result = manager.set_resolution(1920, 1080);
        assert!(result.is_ok());
    }

    #[test]
    fn test_gamma_clamping() {
        let mut manager = DisplayManager::new();
        manager.set_gamma(3.0);
        assert_eq!(manager.config.gamma, 2.0); // Should be clamped

        manager.set_gamma(0.1);
        assert_eq!(manager.config.gamma, 0.5); // Should be clamped
    }

    #[test]
    fn test_monitor_resolutions() {
        let monitor = Monitor::new(0);
        let resolutions = monitor.get_resolutions();
        assert!(!resolutions.is_empty());
    }

    #[test]
    fn test_custom_modes() {
        let mut manager = DisplayManager::new();
        let custom_mode = DisplayMode::new(3440, 1440, 32, 144);
        manager.add_custom_mode(0, custom_mode);

        let all_modes = manager.get_all_modes(0);
        assert!(all_modes.contains(&custom_mode));
    }
}
