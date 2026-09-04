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

fn load_fontdue_font(desc: &FontDesc) -> Option<fontdue::Font> {
    for path in candidate_font_paths(&desc.name) {
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(font) = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()) {
                return Some(font);
            }
        }
    }
    None
}

/// Win32 `MulDiv(point, 96, 72)` — point size to pixel em height at 96 dpi.
///
/// C++ parity: `FontCharsClass::Create_GDI_Font` sizes the GDI font with
/// `-MulDiv(PointSize, 96, 72)` (GeneralsMD/Code/Libraries/Source/WWVegas/
/// WW3D2/render2dsentence.cpp:1491-1492, `const int dotsPerInch = 96; // always use 96.`).
/// Arial-10 is a 13 px em, Arial-12 a 16 px em — never the bare point size.
pub fn font_pixel_size(point_size: i32) -> i32 {
    (point_size * 96 + 36) / 72
}

/// Concrete font files registered into the renderer's glyph atlas font
/// database (C++ GDI resolved the family by name; these are the same files
/// the measuring side prefers, so measure and raster share one face).
pub fn font_atlas_files() -> Vec<std::path::PathBuf> {
    let names = [
        "Arial.ttf",
        "Arial Bold.ttf",
        "arial.ttf",
        "LiberationSans-Regular.ttf",
        "LiberationSans-Bold.ttf",
        "DejaVuSans.ttf",
        "DejaVuSans-Bold.ttf",
    ];
    let dirs = candidate_font_dirs();
    let mut files = Vec::new();
    for dir in dirs {
        for name in names {
            files.push(dir.join(name));
        }
    }
    files
}

fn candidate_font_dirs() -> Vec<std::path::PathBuf> {
    [
        std::path::PathBuf::from("Data/English/Fonts"),
        std::path::PathBuf::from("Data/Fonts"),
        std::path::PathBuf::from("/System/Library/Fonts/Supplemental"),
        std::path::PathBuf::from("/Library/Fonts"),
        std::path::PathBuf::from("/System/Library/Fonts"),
        std::path::PathBuf::from("C:/Windows/Fonts"),
        std::path::PathBuf::from("/usr/share/fonts/truetype"),
        std::path::PathBuf::from("/usr/share/fonts"),
    ]
    .into_iter()
    .collect()
}
 
 fn candidate_font_paths(name: &str) -> Vec<std::path::PathBuf> {
     let mut paths = Vec::new();
     let file_stem = name.replace(' ', "");
     let names = [
         format!("{}.ttf", name),
         format!("{}.otf", name),
         format!("{}.ttf", file_stem),
         format!("{}.TTF", name),
         format!("{} Bold.ttf", name),
     ];
    let dirs = candidate_font_dirs();
     for dir in dirs {
         for file in &names {
             paths.push(dir.join(file));
         }
         paths.push(dir.join("Arial.ttf"));
         paths.push(dir.join("arial.ttf"));
         paths.push(dir.join("LiberationSans-Regular.ttf"));
         paths.push(dir.join("DejaVuSans.ttf"));
     }
     paths
 }

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

/// Font data backed by fontdue glyph advances (C++ GetTextExtentPoint32).
pub struct DefaultFontData {
    metrics: FontMetrics,
    desc: FontDesc,
    font: Option<fontdue::Font>,
}

impl DefaultFontData {
    pub fn new(desc: FontDesc) -> Self {
        let font = load_fontdue_font(&desc);
        // C++ GDI renders point sizes at MulDiv(point, 96, 72) pixel em
        // (render2dsentence.cpp:1492); every metric below is a pixel metric
        // of that em, matching GetTextExtentPoint32/tmHeight behavior.
        let px = font_pixel_size(desc.size) as f32;
        let mut metrics = FontMetrics::default();
        if let Some(loaded) = font.as_ref() {
            if let Some(line) = loaded.horizontal_line_metrics(px) {
                metrics.ascent = line.ascent.round() as i32;
                metrics.descent = line.descent.round() as i32;
                metrics.line_gap = line.line_gap.round() as i32;
            }
            let metrics_x = loaded.metrics('x', px);
            metrics.average_width = metrics_x.advance_width.round().max(1.0) as i32;
            let metrics_m = loaded.metrics('M', px);
            metrics.max_width = metrics_m
                .advance_width
                .round()
                .max(metrics.average_width as f32) as i32;
        } else {
            metrics.ascent = (px * 0.9) as i32;
            metrics.descent = -(px * 0.21) as i32;
            metrics.average_width = (px * 0.6) as i32;
            metrics.max_width = px as i32;
        }
        // C++ `font->height = fontChar->Get_Char_Height()` = GDI tmHeight —
        // the full ascent+descent cell (W3DGameFont.cpp:71,
        // render2dsentence.cpp:1564). Line stacking also walks this cell
        // (render2dsentence.cpp:1115, 574).
        metrics.height = (metrics.ascent - metrics.descent).max(px as i32);

        Self {
            metrics,
            desc,
            font,
        }
    }
}
impl FontData for DefaultFontData {
    fn get_metrics(&self) -> FontMetrics {
        self.metrics.clone()
    }

    fn measure_text(&self, text: &str) -> i32 {
        if let Some(font) = self.font.as_ref() {
            let px = font_pixel_size(self.desc.size) as f32;
            let width = text
                .chars()
                .map(|ch| font.metrics(ch, px).advance_width)
                .sum::<f32>();
            return width.round().max(0.0) as i32;
        }
        text.chars().count() as i32 * self.metrics.average_width.max(1)
    }

    fn get_line_height(&self) -> i32 {
        // C++ wrapped-line pitch is CharHeight (tmHeight) — render2dsentence
        // advances rows by the cell, not by an external-leading-augmented pitch.
        self.metrics.height
    }

    fn supports_char(&self, ch: char) -> bool {
        if let Some(font) = self.font.as_ref() {
            return font.lookup_glyph_index(ch) != 0;
        }
        ch.is_ascii() || ch.is_ascii_graphic() || ch.is_whitespace()
    }
}

/// Game font representation - device independent font object
pub struct GameFont {
    /// Font description
    pub desc: FontDesc,
    /// Pixel height of the font (C++ `GameFont::height` = the glyph atlas'
    /// `Get_Char_Height()` cell, W3DGameFont.cpp:71)
    pub height: i32,
    /// Platform-specific font data
    pub font_data: Box<dyn FontData>,
}

impl GameFont {
    /// Create a new GameFont with the specified description
    pub fn new(desc: FontDesc) -> Result<Self, FontError> {
        let font_data = Box::new(DefaultFontData::new(desc.clone()));
        // C++: `font->height = fontChar->Get_Char_Height()` (W3DGameFont.cpp:71)
        // — the pixel cell height, not the point size.
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

        let mut cache = self.font_cache.lock().unwrap_or_else(|e| e.into_inner());
        let mut order = self.font_order.lock().unwrap_or_else(|e| e.into_inner());

        // Check if font is already cached
        if let Some(weak_font) = cache.get(desc) {
            if let Some(font) = weak_font.upgrade() {
                *self.cache_hits.lock().unwrap_or_else(|e| e.into_inner()) += 1;
                return Ok(font);
            } else {
                // Weak reference is dead, remove it
                cache.remove(desc);
                order.retain(|entry| entry != desc);
            }
        }

        // Font not in cache or weak reference is dead, load it
        *self.cache_misses.lock().unwrap_or_else(|e| e.into_inner()) += 1;

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
        let cache = self.font_cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.keys().next().cloned()
    }

    /// Get all font descriptions currently loaded
    pub fn get_loaded_fonts(&self) -> Vec<FontDesc> {
        let cache = self.font_cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.keys().cloned().collect()
    }

    /// Get the number of fonts currently cached
    pub fn get_count(&self) -> usize {
        let order = self.font_order.lock().unwrap_or_else(|e| e.into_inner());
        order.len()
    }

    /// Clean up dead weak references from the cache
    pub fn cleanup_cache(&mut self) {
        let mut cache = self.font_cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.retain(|_, weak_ref| weak_ref.strong_count() > 0);
        let mut order = self.font_order.lock().unwrap_or_else(|e| e.into_inner());
        order.retain(|desc| {
            cache
                .get(desc)
                .map(|weak_ref| weak_ref.strong_count() > 0)
                .unwrap_or(false)
        });
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> (u64, u64) {
        let hits = *self.cache_hits.lock().unwrap_or_else(|e| e.into_inner());
        let misses = *self.cache_misses.lock().unwrap_or_else(|e| e.into_inner());
        (hits, misses)
    }

    /// Clear all fonts from the cache
    pub fn clear_cache(&mut self) {
        let mut cache = self.font_cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.clear();
        let mut order = self.font_order.lock().unwrap_or_else(|e| e.into_inner());
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
        let order = self.font_order.lock().unwrap_or_else(|e| e.into_inner());
        let desc = order.first()?.clone();
        drop(order);
        self.get_font(&desc).ok()
    }

    /// Return the next font after the provided font description.
    pub fn next_font(&mut self, current: &FontDesc) -> Option<Arc<GameFont>> {
        self.cleanup_cache();
        let order = self.font_order.lock().unwrap_or_else(|e| e.into_inner());
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
            let mut cache = self.font_cache.lock().unwrap_or_else(|e| e.into_inner());
            cache.clear();
        }

        *self.cache_hits.lock().unwrap_or_else(|e| e.into_inner()) = 0;
        *self.cache_misses.lock().unwrap_or_else(|e| e.into_inner()) = 0;

        log::info!("Font library reset successfully");
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Periodic cleanup of dead weak references using interior mutability
        {
            let mut cache = self.font_cache.lock().unwrap_or_else(|e| e.into_inner());
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
        *self.cache_hits.lock().unwrap_or_else(|e| e.into_inner()) = 0;
        *self.cache_misses.lock().unwrap_or_else(|e| e.into_inner()) = 0;

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
static FONT_LIBRARY: std::sync::LazyLock<std::sync::Mutex<FontLibrary>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(FontLibrary::new()));


pub fn get_font_library() -> std::sync::MutexGuard<'static, FontLibrary> {
    FONT_LIBRARY.lock().unwrap_or_else(|e| e.into_inner())
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
        // C++ GameFont::height = Get_Char_Height() pixel cell (W3DGameFont.cpp:71),
        // always >= the MulDiv(point, 96, 72) pixel em.
        assert_eq!(font.height, font.get_metrics().height);
        assert!(font.height >= font_pixel_size(14));
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

        assert!(metrics.height >= font_pixel_size(16));
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
