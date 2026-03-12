//! Utility functions for game development tools

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// File utilities
pub mod file {
    use super::*;

    /// Get the file extension as a lowercase string
    pub fn get_extension(path: &Path) -> Option<String> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.to_lowercase())
    }

    /// Check if a path is a supported image format
    pub fn is_image_file(path: &Path) -> bool {
        if let Some(ext) = get_extension(path) {
            matches!(
                ext.as_str(),
                "png" | "jpg" | "jpeg" | "bmp" | "tga" | "dds" | "gif"
            )
        } else {
            false
        }
    }

    /// Check if a path is a supported 3D model format
    pub fn is_model_file(path: &Path) -> bool {
        if let Some(ext) = get_extension(path) {
            matches!(
                ext.as_str(),
                "w3d" | "obj" | "fbx" | "3ds" | "max" | "dae" | "blend"
            )
        } else {
            false
        }
    }

    /// Check if a path is a supported audio format
    pub fn is_audio_file(path: &Path) -> bool {
        if let Some(ext) = get_extension(path) {
            matches!(ext.as_str(), "wav" | "mp3" | "ogg" | "flac" | "aac")
        } else {
            false
        }
    }

    /// Get file size in human-readable format
    pub fn format_file_size(size: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = size as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{:.0} {}", size, UNITS[unit_index])
        } else {
            format!("{:.1} {}", size, UNITS[unit_index])
        }
    }

    /// Get relative path from base to target
    pub fn get_relative_path(base: &Path, target: &Path) -> Result<PathBuf> {
        target
            .strip_prefix(base)
            .map(|p| p.to_path_buf())
            .or_else(|_| {
                // If strip_prefix fails, try to find a common ancestor
                let base_components: Vec<_> = base.components().collect();
                let target_components: Vec<_> = target.components().collect();

                let mut common_len = 0;
                for (b, t) in base_components.iter().zip(target_components.iter()) {
                    if b == t {
                        common_len += 1;
                    } else {
                        break;
                    }
                }

                let mut result = PathBuf::new();

                // Add ".." for each non-common component in base
                for _ in common_len..base_components.len() {
                    result.push("..");
                }

                // Add remaining components from target
                for component in &target_components[common_len..] {
                    result.push(component);
                }

                Ok(result)
            })
    }

    /// Create directory if it doesn't exist
    pub async fn ensure_directory(path: &Path) -> Result<()> {
        if !path.exists() {
            tokio::fs::create_dir_all(path).await?;
        }
        Ok(())
    }
}

/// Math utilities
pub mod math {
    use glam::{Mat4, Vec2, Vec3, Vec4};

    /// Clamp a value between min and max
    pub fn clamp<T: PartialOrd>(value: T, min: T, max: T) -> T {
        if value < min {
            min
        } else if value > max {
            max
        } else {
            value
        }
    }

    /// Linear interpolation
    pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
        a + (b - a) * t
    }

    /// Smoothstep interpolation
    pub fn smoothstep(a: f32, b: f32, t: f32) -> f32 {
        let t = clamp((t - a) / (b - a), 0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// Convert degrees to radians
    pub fn deg_to_rad(degrees: f32) -> f32 {
        degrees * std::f32::consts::PI / 180.0
    }

    /// Convert radians to degrees
    pub fn rad_to_deg(radians: f32) -> f32 {
        radians * 180.0 / std::f32::consts::PI
    }

    /// Distance between two 2D points
    pub fn distance_2d(a: Vec2, b: Vec2) -> f32 {
        (b - a).length()
    }

    /// Distance between two 3D points
    pub fn distance_3d(a: Vec3, b: Vec3) -> f32 {
        (b - a).length()
    }

    /// Check if a point is inside a 2D rectangle
    pub fn point_in_rect(point: Vec2, rect_min: Vec2, rect_max: Vec2) -> bool {
        point.x >= rect_min.x
            && point.x <= rect_max.x
            && point.y >= rect_min.y
            && point.y <= rect_max.y
    }
}

/// Color utilities
pub mod color {
    use eframe::egui;

    /// Convert RGB to HSV
    pub fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
        let max = r.max(g.max(b));
        let min = r.min(g.min(b));
        let delta = max - min;

        let h = if delta == 0.0 {
            0.0
        } else if max == r {
            60.0 * ((g - b) / delta % 6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };

        let s = if max == 0.0 { 0.0 } else { delta / max };
        let v = max;

        (h, s, v)
    }

    /// Convert HSV to RGB
    pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;

        let (r_prime, g_prime, b_prime) = match h as i32 / 60 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            5 => (c, 0.0, x),
            _ => (0.0, 0.0, 0.0),
        };

        (r_prime + m, g_prime + m, b_prime + m)
    }

    /// Create an egui color from RGB values (0-255)
    pub fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 {
        egui::Color32::from_rgb(r, g, b)
    }

    /// Create an egui color from RGBA values (0-255)
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> egui::Color32 {
        egui::Color32::from_rgba_unmultiplied(r, g, b, a)
    }

    /// Create an egui color from hex string (e.g., "#FF0000")
    pub fn from_hex(hex: &str) -> Result<egui::Color32, String> {
        let hex = hex.trim_start_matches('#');

        if hex.len() != 6 && hex.len() != 8 {
            return Err("Invalid hex color format".to_string());
        }

        let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid hex color")?;
        let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid hex color")?;
        let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid hex color")?;

        if hex.len() == 8 {
            let a = u8::from_str_radix(&hex[6..8], 16).map_err(|_| "Invalid hex color")?;
            Ok(egui::Color32::from_rgba_unmultiplied(r, g, b, a))
        } else {
            Ok(egui::Color32::from_rgb(r, g, b))
        }
    }
}

/// String utilities
pub mod string {
    /// Truncate string to specified length with ellipsis
    pub fn truncate(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else if max_len < 3 {
            "...".to_string()
        } else {
            format!("{}...", &s[..max_len - 3])
        }
    }

    /// Convert camelCase or PascalCase to Title Case
    pub fn camel_to_title(s: &str) -> String {
        let mut result = String::new();
        let mut prev_was_lower = false;

        for (i, c) in s.char_indices() {
            if i == 0 {
                result.push(c.to_uppercase().next().unwrap_or(c));
            } else if c.is_uppercase() && prev_was_lower {
                result.push(' ');
                result.push(c);
            } else {
                result.push(c);
            }

            prev_was_lower = c.is_lowercase();
        }

        result
    }

    /// Convert string to valid filename (remove invalid characters)
    pub fn to_filename(s: &str) -> String {
        s.chars()
            .map(|c| match c {
                '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
                '/' | '\\' => '_',
                c if c.is_control() => '_',
                c => c,
            })
            .collect()
    }
}

/// Time utilities
pub mod time {
    use super::*;

    /// Get current timestamp as seconds since Unix epoch
    pub fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Format duration in human-readable format
    pub fn format_duration(seconds: u64) -> String {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        let seconds = seconds % 60;

        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }

    /// Format timestamp as human-readable date/time
    pub fn format_timestamp(timestamp: u64) -> String {
        use std::time::{Duration, SystemTime};

        let system_time = UNIX_EPOCH + Duration::from_secs(timestamp);
        format!("{:?}", system_time) // Simple formatting, could use chrono for better formatting
    }
}

/// UUID utilities
pub mod uuid {
    use uuid::Uuid;

    /// Generate a new UUID v4
    pub fn generate() -> String {
        Uuid::new_v4().to_string()
    }

    /// Generate a short UUID (first 8 characters)
    pub fn generate_short() -> String {
        generate().split('-').next().unwrap_or("").to_string()
    }

    /// Validate UUID string
    pub fn is_valid(uuid_str: &str) -> bool {
        Uuid::parse_str(uuid_str).is_ok()
    }
}

/// Performance utilities
pub mod perf {
    use std::time::Instant;

    /// Simple performance timer
    pub struct Timer {
        start: Instant,
        name: String,
    }

    impl Timer {
        pub fn new(name: &str) -> Self {
            Self {
                start: Instant::now(),
                name: name.to_string(),
            }
        }

        pub fn elapsed_ms(&self) -> f64 {
            self.start.elapsed().as_secs_f64() * 1000.0
        }

        pub fn lap(&mut self, label: &str) {
            let elapsed = self.elapsed_ms();
            log::debug!("{} - {}: {:.2}ms", self.name, label, elapsed);
            self.start = Instant::now();
        }
    }

    impl Drop for Timer {
        fn drop(&mut self) {
            let elapsed = self.elapsed_ms();
            log::debug!("{} completed in {:.2}ms", self.name, elapsed);
        }
    }

    /// Performance profiler for tracking function call times
    pub struct Profiler {
        entries: std::collections::HashMap<String, ProfileEntry>,
    }

    impl Profiler {
        pub fn new() -> Self {
            Self {
                entries: std::collections::HashMap::new(),
            }
        }

        pub fn start(&mut self, name: &str) {
            self.entries.insert(
                name.to_string(),
                ProfileEntry {
                    start: Instant::now(),
                    total_time: std::time::Duration::ZERO,
                    call_count: 0,
                },
            );
        }

        pub fn end(&mut self, name: &str) {
            if let Some(entry) = self.entries.get_mut(name) {
                let elapsed = entry.start.elapsed();
                entry.total_time += elapsed;
                entry.call_count += 1;
            }
        }

        pub fn report(&self) -> String {
            let mut report = String::from("Performance Report:\n");

            for (name, entry) in &self.entries {
                let avg_time = entry.total_time.as_secs_f64() / entry.call_count as f64;
                report.push_str(&format!(
                    "  {}: {:.2}ms avg ({} calls, {:.2}ms total)\n",
                    name,
                    avg_time * 1000.0,
                    entry.call_count,
                    entry.total_time.as_secs_f64() * 1000.0
                ));
            }

            report
        }
    }

    struct ProfileEntry {
        start: Instant,
        total_time: std::time::Duration,
        call_count: u32,
    }
}
