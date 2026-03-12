//! # Font Management System
//!
//! This module provides device-independent font management for the game client.
//! It handles font loading, caching, and provides metrics for text rendering.
//!
//! ## Features
//! - Font loading and caching system
//! - Support for different font sizes, styles, and effects
//! - Device-independent font representations
//! - Efficient font library management with reference counting
//! - Memory pool integration for optimal performance
//!
//! ## Usage
//! ```rust
//! use crate::gui::font::{FontLibrary, FontDesc};
//!
//! let mut font_library = FontLibrary::new();
//! font_library.init()?;
//!
//! let font_desc = FontDesc::new("Arial", 12, false);
//! let font = font_library.get_font(&font_desc)?;
//! ```

use crate::system::SubsystemInterface;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use thiserror::Error;

/// Font management errors
#[derive(Error, Debug)]
pub enum FontError {
    #[error("Font not found: {name} size {size}")]
    FontNotFound { name: String, size: i32 },
    #[error("Failed to load font data: {0}")]
    LoadError(String),
    #[error("Font library not initialized")]
    NotInitialized,
    #[error("Invalid font parameters: {0}")]
    InvalidParameters(String),
}

/// Font description structure for specifying font requirements
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FontDesc {
    /// Name of the font family
    pub name: String,
    /// Point size of the font
    pub size: i32,
    /// Whether the font should be bold
    pub bold: bool,
}

impl FontDesc {
    /// Create a new font description
    ///
    /// # Arguments
    /// * `name` - Font family name
    /// * `size` - Point size of the font
    /// * `bold` - Whether the font should be bold
    pub fn new(name: &str, size: i32, bold: bool) -> Self {
        Self {
            name: name.to_string(),
            size,
            bold,
        }
    }
}

impl Default for FontDesc {
    fn default() -> Self {
        Self {
            name: "Arial".to_string(),
            size: 12,
            bold: false,
        }
    }
}

/// Font metrics for layout calculations
#[derive(Debug, Clone)]
pub struct FontMetrics {
    /// Pixel height of the font
    pub height: i32,
    /// Ascender height (baseline to top)
    pub ascent: i32,
    /// Descender height (baseline to bottom, typically negative)
    pub descent: i32,
    /// Line gap spacing
    pub line_gap: i32,
    /// Average character width
    pub average_width: i32,
    /// Maximum character width
    pub max_width: i32,
}

impl Default for FontMetrics {
    fn default() -> Self {
        Self {
            height: 12,
            ascent: 10,
            descent: -2,
            line_gap: 2,
            average_width: 8,
            max_width: 12,
        }
    }
}

/// Platform-specific font data
pub trait FontData: Send + Sync {
    /// Get font metrics
    fn get_metrics(&self) -> FontMetrics;

    /// Measure the width of a text string
    fn measure_text(&self, text: &str) -> i32;

    /// Get the font's line height
    fn get_line_height(&self) -> i32;

    /// Check if a character is supported by this font
    fn supports_char(&self, ch: char) -> bool;
}

/// Default font data implementation for testing and fallback
#[derive(Debug)]
pub struct DefaultFontData {
    metrics: FontMetrics,
    desc: FontDesc,
}

impl DefaultFontData {
    pub fn new(desc: FontDesc) -> Self {
        let mut metrics = FontMetrics::default();
        metrics.height = desc.size;
        metrics.ascent = (desc.size as f32 * 0.8) as i32;
        metrics.descent = -(desc.size as f32 * 0.2) as i32;
        metrics.average_width = (desc.size as f32 * 0.6) as i32;
        metrics.max_width = desc.size;

        Self { metrics, desc }
    }
}

impl FontData for DefaultFontData {
    fn get_metrics(&self) -> FontMetrics {
        self.metrics.clone()
    }

    fn measure_text(&self, text: &str) -> i32 {
        text.len() as i32 * self.metrics.average_width
    }

    fn get_line_height(&self) -> i32 {
        self.metrics.height + self.metrics.line_gap
    }

    fn supports_char(&self, ch: char) -> bool {
        ch.is_ascii() || ch.is_ascii_graphic() || ch.is_whitespace()
    }
}

/// Game font representation - device independent font object
pub struct GameFont {
    /// Font description
    pub desc: FontDesc,
    /// Pixel height of the font (derived from point size)
    pub height: i32,
    /// Platform-specific font data
    pub font_data: Box<dyn FontData>,
}

impl GameFont {
    /// Create a new GameFont with the specified description
    pub fn new(desc: FontDesc) -> Result<Self, FontError> {
        // For now, use default font data - in a real implementation,
        // this would load platform-specific font data
        let font_data = Box::new(DefaultFontData::new(desc.clone()));
        let height = font_data.get_metrics().height;

        Ok(Self {
            desc,
            height,
            font_data,
        })
    }

    /// Get font metrics
    pub fn get_metrics(&self) -> FontMetrics {
        self.font_data.get_metrics()
    }

    /// Measure the width of text when rendered with this font
    pub fn measure_text(&self, text: &str) -> i32 {
        self.font_data.measure_text(text)
    }

    /// Get the line height for this font
    pub fn get_line_height(&self) -> i32 {
        self.font_data.get_line_height()
    }

    /// Check if this font supports a specific character
    pub fn supports_char(&self, ch: char) -> bool {
        self.font_data.supports_char(ch)
    }
}

/// Font library for managing loaded fonts
///
/// This provides a centralized system for loading, caching, and accessing fonts.
/// Fonts are cached and reference-counted to avoid duplicate loading.
pub struct FontLibrary {
    /// Cache of loaded fonts, keyed by FontDesc
    font_cache: Arc<Mutex<HashMap<FontDesc, Weak<GameFont>>>>,
    /// Insertion-ordered list of loaded fonts
    font_order: Arc<Mutex<Vec<FontDesc>>>,
    /// Whether the library has been initialized
    initialized: bool,
    /// Statistics for debugging and monitoring
    cache_hits: Arc<Mutex<u64>>,
    cache_misses: Arc<Mutex<u64>>,
}

impl FontLibrary {
    /// Create a new font library
    pub fn new() -> Self {
        Self {
            font_cache: Arc::new(Mutex::new(HashMap::new())),
            font_order: Arc::new(Mutex::new(Vec::new())),
            initialized: false,
            cache_hits: Arc::new(Mutex::new(0)),
            cache_misses: Arc::new(Mutex::new(0)),
        }
    }

    /// Get a font matching the specified description
    ///
    /// This method will first check the cache, and if not found, will load
    /// the font and add it to the cache.
    ///
    /// # Arguments
    /// * `desc` - Font description specifying the desired font
    ///
    /// # Returns
    /// * `Ok(Arc<GameFont>)` - Shared reference to the font
    /// * `Err(FontError)` - If the font cannot be loaded
    pub fn get_font(&mut self, desc: &FontDesc) -> Result<Arc<GameFont>, FontError> {
        if !self.initialized {
            return Err(FontError::NotInitialized);
        }

        // Validate parameters
        if desc.name.is_empty() || desc.size <= 0 {
            return Err(FontError::InvalidParameters(format!(
                "Invalid font parameters: name='{}', size={}",
                desc.name, desc.size
            )));
        }

        let mut cache = self.font_cache.lock().unwrap();
        let mut order = self.font_order.lock().unwrap();

        // Check if font is already cached
        if let Some(weak_font) = cache.get(desc) {
            if let Some(font) = weak_font.upgrade() {
                *self.cache_hits.lock().unwrap() += 1;
                return Ok(font);
            } else {
                // Weak reference is dead, remove it
                cache.remove(desc);
                order.retain(|entry| entry != desc);
            }
        }

        // Font not in cache or weak reference is dead, load it
        *self.cache_misses.lock().unwrap() += 1;

        let game_font =
            GameFont::new(desc.clone()).map_err(|e| FontError::LoadError(e.to_string()))?;

        let font_arc = Arc::new(game_font);
        cache.insert(desc.clone(), Arc::downgrade(&font_arc));
        if !order.contains(desc) {
            order.push(desc.clone());
        }

        Ok(font_arc)
    }

    /// Get the first loaded font description.
    pub fn first_font_desc(&self) -> Option<FontDesc> {
        let cache = self.font_cache.lock().unwrap();
        cache.keys().next().cloned()
    }

    /// Get all font descriptions currently loaded
    pub fn get_loaded_fonts(&self) -> Vec<FontDesc> {
        let cache = self.font_cache.lock().unwrap();
        cache.keys().cloned().collect()
    }

    /// Get the number of fonts currently cached
    pub fn get_count(&self) -> usize {
        let order = self.font_order.lock().unwrap();
        order.len()
    }

    /// Clean up dead weak references from the cache
    pub fn cleanup_cache(&mut self) {
        let mut cache = self.font_cache.lock().unwrap();
        cache.retain(|_, weak_ref| weak_ref.strong_count() > 0);
        let mut order = self.font_order.lock().unwrap();
        order.retain(|desc| {
            cache
                .get(desc)
                .map(|weak_ref| weak_ref.strong_count() > 0)
                .unwrap_or(false)
        });
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> (u64, u64) {
        let hits = *self.cache_hits.lock().unwrap();
        let misses = *self.cache_misses.lock().unwrap();
        (hits, misses)
    }

    /// Clear all fonts from the cache
    pub fn clear_cache(&mut self) {
        let mut cache = self.font_cache.lock().unwrap();
        cache.clear();
        let mut order = self.font_order.lock().unwrap();
        order.clear();
    }

    /// C++-style font lookup by name/size/bold.
    pub fn get_font_by_name(
        &mut self,
        name: &str,
        point_size: i32,
        bold: bool,
    ) -> Result<Arc<GameFont>, FontError> {
        let desc = FontDesc::new(name, point_size, bold);
        self.get_font(&desc)
    }

    /// Return the first font in insertion order.
    pub fn first_font(&mut self) -> Option<Arc<GameFont>> {
        self.cleanup_cache();
        let order = self.font_order.lock().unwrap();
        let desc = order.first()?.clone();
        drop(order);
        self.get_font(&desc).ok()
    }

    /// Return the next font after the provided font description.
    pub fn next_font(&mut self, current: &FontDesc) -> Option<Arc<GameFont>> {
        self.cleanup_cache();
        let order = self.font_order.lock().unwrap();
        let index = order.iter().position(|desc| desc == current)?;
        let next = order.get(index + 1)?.clone();
        drop(order);
        self.get_font(&next).ok()
    }
}

impl Default for FontLibrary {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for FontLibrary {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Initializing font library");
        log::info!("Font library initialized successfully");
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Resetting font library");

        // Clear cache using interior mutability
        {
            let mut cache = self.font_cache.lock().unwrap();
            cache.clear();
        }

        *self.cache_hits.lock().unwrap() = 0;
        *self.cache_misses.lock().unwrap() = 0;

        log::info!("Font library reset successfully");
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Periodic cleanup of dead weak references using interior mutability
        {
            let mut cache = self.font_cache.lock().unwrap();
            cache.retain(|_, weak_ref| weak_ref.strong_count() > 0);
        }
        Ok(())
    }
}

impl FontLibrary {
    /// Initialize the font library (mutable version for direct initialization)
    pub fn init_mut(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Initializing font library");

        // In a real implementation, this would initialize platform-specific
        // font loading systems (DirectWrite, FreeType, etc.)
        self.initialized = true;

        log::info!("Font library initialized successfully");
        Ok(())
    }

    /// Reset the font library (mutable version)
    pub fn reset_mut(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Resetting font library");

        self.clear_cache();
        *self.cache_hits.lock().unwrap() = 0;
        *self.cache_misses.lock().unwrap() = 0;

        log::info!("Font library reset successfully");
        Ok(())
    }

    /// Update the font library (mutable version)
    pub fn update_mut(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Periodic cleanup of dead weak references
        self.cleanup_cache();
        Ok(())
    }

    /// Shutdown the font library
    pub fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Shutting down font library");

        let (hits, misses) = self.get_cache_stats();
        log::info!(
            "Font library cache stats - Hits: {}, Misses: {}",
            hits,
            misses
        );

        self.clear_cache();
        self.initialized = false;

        log::info!("Font library shutdown completed");
        Ok(())
    }
}

/// Global font library instance
static FONT_LIBRARY: std::sync::OnceLock<std::sync::Mutex<FontLibrary>> =
    std::sync::OnceLock::new();

/// Get the global font library instance
pub fn get_font_library() -> std::sync::MutexGuard<'static, FontLibrary> {
    let lock = FONT_LIBRARY.get_or_init(|| std::sync::Mutex::new(FontLibrary::new()));
    lock.lock().expect("FontLibrary mutex poisoned")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_desc_creation() {
        let desc = FontDesc::new("Arial", 12, false);
        assert_eq!(desc.name, "Arial");
        assert_eq!(desc.size, 12);
        assert!(!desc.bold);
    }

    #[test]
    fn test_font_desc_default() {
        let desc = FontDesc::default();
        assert_eq!(desc.name, "Arial");
        assert_eq!(desc.size, 12);
        assert!(!desc.bold);
    }

    #[test]
    fn test_game_font_creation() {
        let desc = FontDesc::new("Times New Roman", 14, true);
        let font = GameFont::new(desc.clone()).unwrap();
        assert_eq!(font.desc, desc);
        assert_eq!(font.height, 14);
    }

    #[test]
    fn test_font_library_init() {
        let mut library = FontLibrary::new();
        assert!(library.init().is_ok());
        assert!(library.initialized);
    }

    #[test]
    fn test_font_library_get_font_before_init() {
        let mut library = FontLibrary::new();
        let desc = FontDesc::new("Arial", 12, false);
        let result = library.get_font(&desc);
        assert!(matches!(result, Err(FontError::NotInitialized)));
    }

    #[test]
    fn test_font_library_get_font_invalid_params() {
        let mut library = FontLibrary::new();
        library.init().unwrap();

        let desc = FontDesc::new("", 12, false);
        let result = library.get_font(&desc);
        assert!(matches!(result, Err(FontError::InvalidParameters(_))));

        let desc = FontDesc::new("Arial", 0, false);
        let result = library.get_font(&desc);
        assert!(matches!(result, Err(FontError::InvalidParameters(_))));
    }

    #[test]
    fn test_font_library_caching() {
        let mut library = FontLibrary::new();
        library.init().unwrap();

        let desc = FontDesc::new("Arial", 12, false);

        // First call should be a cache miss
        let font1 = library.get_font(&desc).unwrap();
        let (hits, misses) = library.get_cache_stats();
        assert_eq!(hits, 0);
        assert_eq!(misses, 1);

        // Second call should be a cache hit
        let font2 = library.get_font(&desc).unwrap();
        let (hits, misses) = library.get_cache_stats();
        assert_eq!(hits, 1);
        assert_eq!(misses, 1);

        // Should be the same Arc
        assert!(Arc::ptr_eq(&font1, &font2));
    }

    #[test]
    fn test_font_metrics() {
        let desc = FontDesc::new("Arial", 16, false);
        let font = GameFont::new(desc).unwrap();
        let metrics = font.get_metrics();

        assert_eq!(metrics.height, 16);
        assert!(metrics.ascent > 0);
        assert!(metrics.descent <= 0);
        assert!(metrics.average_width > 0);
    }

    #[test]
    fn test_text_measurement() {
        let desc = FontDesc::new("Arial", 12, false);
        let font = GameFont::new(desc).unwrap();

        let width1 = font.measure_text("Hello");
        let width2 = font.measure_text("Hello World");

        assert!(width1 > 0);
        assert!(width2 > width1);
    }

    #[test]
    fn test_font_library_cleanup() {
        let mut library = FontLibrary::new();
        library.init().unwrap();

        let desc = FontDesc::new("Arial", 12, false);

        {
            let _font = library.get_font(&desc).unwrap();
            assert_eq!(library.get_count(), 1);
        } // font goes out of scope here

        library.cleanup_cache();
        assert_eq!(library.get_count(), 0);
    }

    #[test]
    fn test_font_library_reset() {
        let mut library = FontLibrary::new();
        library.init().unwrap();

        let desc = FontDesc::new("Arial", 12, false);
        let _font = library.get_font(&desc).unwrap();

        assert_eq!(library.get_count(), 1);
        let (hits, misses) = library.get_cache_stats();
        assert!(misses > 0);

        library.reset().unwrap();

        assert_eq!(library.get_count(), 0);
        let (hits, misses) = library.get_cache_stats();
        assert_eq!(hits, 0);
        assert_eq!(misses, 0);
    }

    #[test]
    fn test_global_font_library() {
        let mut library = get_font_library();
        assert!(library.init().is_ok());

        let desc = FontDesc::new("Arial", 12, false);
        let font = library.get_font(&desc).unwrap();
        assert_eq!(font.desc.name, "Arial");
    }
}
