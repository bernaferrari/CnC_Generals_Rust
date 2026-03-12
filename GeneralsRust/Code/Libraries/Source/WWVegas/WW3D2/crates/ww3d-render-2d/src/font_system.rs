//! Font System
//!
//! This module manages bitmap fonts for text rendering with full support
//! for TGA font atlases and modern TTF/OTF fonts.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use ww3d_assets::AssetManager;
use ww3d_renderer_3d::rendering::render2d::font3d::{Font3DData, FontChar};
use ww3d_renderer_3d::texture_system::{SurfaceClass, SurfaceRect, TextureClass};

#[derive(Debug, Error)]
pub enum FontError {
    #[error("Font file not found: {0}")]
    FileNotFound(String),

    #[error("Failed to load font image: {0}")]
    ImageLoadError(String),

    #[error("Invalid font format: {0}")]
    InvalidFormat(String),

    #[error("Font already loaded: {0}")]
    AlreadyLoaded(String),

    #[error("Font not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Comprehensive font management system with TGA and TTF support
#[derive(Debug)]
pub struct FontSystem {
    /// Loaded fonts indexed by name
    fonts: HashMap<String, Arc<Font3DData>>,

    /// Asset manager for loading font files
    asset_manager: Option<Arc<AssetManager>>,

    /// Default font name
    default_font: Option<String>,
}

impl FontSystem {
    /// Create a new font system
    pub fn new() -> Self {
        Self {
            fonts: HashMap::new(),
            asset_manager: None,
            default_font: None,
        }
    }

    /// Set the asset manager for loading font files
    pub fn set_asset_manager(&mut self, manager: Arc<AssetManager>) {
        self.asset_manager = Some(manager);
    }

    /// Load a TGA font file and create a font atlas
    ///
    /// TGA fonts are expected to be 16x16 grids of characters (256 total)
    /// or font strikes (horizontal strips). The system will automatically
    /// convert monospace fonts to proportional and optimize the texture.
    pub fn load_tga_font(&mut self, name: &str, path: &str) -> Result<Arc<Font3DData>, FontError> {
        // Check if already loaded
        if self.fonts.contains_key(name) {
            return Err(FontError::AlreadyLoaded(name.to_string()));
        }

        // Load the TGA file
        let surface = self.load_font_surface(path)?;

        // Create font data from the surface
        let font_data = self.create_font_from_tga(name, &surface)?;

        // Store and return
        let font_arc = Arc::new(font_data);
        self.fonts.insert(name.to_string(), font_arc.clone());

        Ok(font_arc)
    }

    /// Load a font from any supported format (TGA, TTF, OTF)
    pub fn load_font(&mut self, name: &str, path: &str) -> Result<Arc<Font3DData>, FontError> {
        let extension = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match extension.to_lowercase().as_str() {
            "tga" => self.load_tga_font(name, path),
            "ttf" | "otf" => self.load_ttf_font(name, path),
            _ => Err(FontError::InvalidFormat(format!(
                "Unsupported font format: {}",
                extension
            ))),
        }
    }

    /// Load a TrueType or OpenType font and generate a bitmap atlas
    ///
    /// This creates a texture atlas at the specified size with all printable
    /// ASCII characters plus common extended characters.
    pub fn load_ttf_font(&mut self, name: &str, path: &str) -> Result<Arc<Font3DData>, FontError> {
        // Check if already loaded
        if self.fonts.contains_key(name) {
            return Err(FontError::AlreadyLoaded(name.to_string()));
        }

        self.load_ttf_font_with_size(name, path, 32.0)
    }

    /// Load a TTF font with a specific font size in pixels
    pub fn load_ttf_font_with_size(
        &mut self,
        name: &str,
        path: &str,
        font_size: f32,
    ) -> Result<Arc<Font3DData>, FontError> {
        // Check if already loaded
        if self.fonts.contains_key(name) {
            return Err(FontError::AlreadyLoaded(name.to_string()));
        }

        // Load the font file
        let font_data = std::fs::read(path)?;

        // Parse the font
        let font = FontRef::try_from_slice(&font_data)
            .map_err(|e| FontError::InvalidFormat(format!("Failed to parse TTF font: {}", e)))?;

        // Generate the font atlas
        let font_3d_data = self.generate_ttf_atlas(name, &font, font_size)?;

        // Store and return
        let font_arc = Arc::new(font_3d_data);
        self.fonts.insert(name.to_string(), font_arc.clone());

        Ok(font_arc)
    }

    /// Generate a bitmap atlas from a TTF font
    fn generate_ttf_atlas(
        &self,
        name: &str,
        font: &FontRef,
        font_size: f32,
    ) -> Result<Font3DData, FontError> {
        // Character set to include in the atlas (printable ASCII + common extended)
        let chars: Vec<char> = (32..127)
            .map(|c| c as u8 as char)
            .chain((128..256).filter_map(|c| char::from_u32(c)))
            .collect();

        // Scale the font
        let scale = PxScale::from(font_size);
        let scaled_font = font.as_scaled(scale);

        // Calculate atlas dimensions
        let chars_per_row = 16;
        let rows = (chars.len() + chars_per_row - 1) / chars_per_row;

        // Measure glyphs to determine cell size
        let mut max_width = 0.0f32;
        let mut max_height = 0.0f32;

        for &ch in &chars {
            let glyph_id = font.glyph_id(ch);
            if !glyph_id.0 == 0 {
                // GlyphId(0) indicates invalid glyph
                let _glyph = glyph_id.with_scale(scale);

                // Get horizontal advance
                let h_advance = scaled_font.h_advance(glyph_id);
                max_width = max_width.max(h_advance);

                // Get vertical metrics
                let v_metrics = scaled_font.height();
                max_height = max_height.max(v_metrics);
            }
        }

        // Add padding
        let cell_width = (max_width.ceil() as u32 + 2).max(16);
        let cell_height = (max_height.ceil() as u32 + 2).max(16);

        let atlas_width = cell_width * chars_per_row as u32;
        let atlas_height = cell_height * rows as u32;

        // Create bitmap buffer (RGBA)
        let mut bitmap = vec![0u8; (atlas_width * atlas_height * 4) as usize];

        // Build character table
        let mut char_table = HashMap::new();

        for (idx, &ch) in chars.iter().enumerate() {
            let col = (idx % chars_per_row) as u32;
            let row = (idx / chars_per_row) as u32;

            let x_offset = col * cell_width;
            let y_offset = row * cell_height;

            let glyph_id = font.glyph_id(ch);
            if glyph_id.0 != 0 {
                // GlyphId(0) indicates invalid glyph
                let glyph = glyph_id.with_scale_and_position(
                    scale,
                    ab_glyph::point(
                        x_offset as f32 + 1.0,
                        y_offset as f32 + 1.0 + scaled_font.ascent(),
                    ),
                );

                // Rasterize the glyph
                if let Some(outlined) = font.outline_glyph(glyph) {
                    let bounds = outlined.px_bounds();
                    let _glyph_width = bounds.width() as u32;
                    let _glyph_height = bounds.height() as u32;

                    outlined.draw(|x, y, coverage| {
                        let px = x_offset + x + 1;
                        let py = y_offset + y;

                        if px < atlas_width && py < atlas_height {
                            let idx = ((py * atlas_width + px) * 4) as usize;
                            let alpha = (coverage * 255.0) as u8;
                            bitmap[idx] = 255; // R
                            bitmap[idx + 1] = 255; // G
                            bitmap[idx + 2] = 255; // B
                            bitmap[idx + 3] = alpha; // A
                        }
                    });

                    // Get horizontal advance for proper spacing
                    let h_advance = scaled_font.h_advance(glyph_id);

                    // Create font character entry
                    let font_char = FontChar {
                        u_offset: (x_offset as f32) / (atlas_width as f32),
                        v_offset: (y_offset as f32) / (atlas_height as f32),
                        u_width: (cell_width as f32) / (atlas_width as f32),
                        v_height: (cell_height as f32) / (atlas_height as f32),
                        width: h_advance.ceil() as u8,
                        height: cell_height as u8,
                    };

                    char_table.insert(ch as u8, font_char);
                }
            }
        }

        // Create surface from bitmap
        use ww3d_renderer_3d::texture_system::TextureFormat;
        let surface = SurfaceClass::from_bytes(
            atlas_width,
            atlas_height,
            TextureFormat::Rgba8Unorm,
            &bitmap,
        )
        .map_err(|e| FontError::ImageLoadError(format!("Failed to create surface: {}", e)))?;

        // Create texture from surface
        let texture = TextureClass::from_surface(name, &surface)
            .map_err(|e| FontError::ImageLoadError(format!("Failed to create texture: {}", e)))?;

        Ok(Font3DData {
            name: name.to_string(),
            texture: Some(texture),
            char_table,
            char_height: cell_height as u8,
            space_width: (max_width * 0.25) as u8,
        })
    }

    /// Get a loaded font by name
    pub fn get_font(&self, name: &str) -> Option<Arc<Font3DData>> {
        self.fonts.get(name).cloned()
    }

    /// Set the default font
    pub fn set_default_font(&mut self, name: &str) -> Result<(), FontError> {
        if !self.fonts.contains_key(name) {
            return Err(FontError::NotFound(name.to_string()));
        }
        self.default_font = Some(name.to_string());
        Ok(())
    }

    /// Get the default font
    pub fn get_default_font(&self) -> Option<Arc<Font3DData>> {
        self.default_font
            .as_ref()
            .and_then(|name| self.fonts.get(name).cloned())
    }

    /// Unload a font by name
    pub fn unload_font(&mut self, name: &str) -> bool {
        self.fonts.remove(name).is_some()
    }

    /// Clear all loaded fonts
    pub fn clear(&mut self) {
        self.fonts.clear();
        self.default_font = None;
    }

    /// Get the number of loaded fonts
    pub fn font_count(&self) -> usize {
        self.fonts.len()
    }

    /// List all loaded font names
    pub fn list_fonts(&self) -> Vec<String> {
        self.fonts.keys().cloned().collect()
    }

    // Private implementation methods

    /// Load a font surface from disk
    fn load_font_surface(&self, path: &str) -> Result<SurfaceClass, FontError> {
        // Try to load via asset manager first
        if let Some(ref _manager) = self.asset_manager {
            // Asset manager integration would go here once exposed.
            // For now, fall through to direct file loading.
        }

        let image = image::open(path)
            .map_err(|e| FontError::ImageLoadError(format!("Failed to open font image: {e}")))?;
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();

        use ww3d_renderer_3d::texture_system::TextureFormat;
        SurfaceClass::from_bytes(width, height, TextureFormat::Rgba8Unorm, rgba.as_raw())
            .map_err(|e| FontError::ImageLoadError(format!("Failed to create surface: {e}")))
    }

    /// Create font data from a TGA surface
    fn create_font_from_tga(
        &self,
        name: &str,
        surface: &SurfaceClass,
    ) -> Result<Font3DData, FontError> {
        let desc = surface.description();
        let width = desc.width as f32;
        let height = desc.height as f32;

        // Determine if this is a 16x16 grid or a font strike
        let is_strike = width > 8.0 * height;

        if is_strike {
            self.create_font_from_strike(name, surface)
        } else {
            self.create_font_from_grid(name, surface)
        }
    }

    /// Create font from a 16x16 grid layout
    fn create_font_from_grid(
        &self,
        name: &str,
        surface: &SurfaceClass,
    ) -> Result<Font3DData, FontError> {
        let desc = surface.description();
        let width = desc.width as f32;
        let height = desc.height as f32;

        // Each character occupies 1/16th of the texture in each dimension
        let char_width = width / 16.0;
        let char_height = height / 16.0;
        let u_step = 1.0 / 16.0;
        let v_step = 1.0 / 16.0;

        let mut char_table = HashMap::new();

        // Build character table
        for char_index in 0..256 {
            let row = (char_index / 16) as f32;
            let col = (char_index % 16) as f32;

            let _u_offset = col * u_step;
            let v_offset = row * v_step;

            // Find the actual character bounds for proportional spacing
            let (actual_width, actual_u_offset, actual_u_width) =
                self.find_char_bounds(surface, col as usize, row as usize, char_width, char_height);

            let font_char = FontChar {
                u_offset: actual_u_offset,
                v_offset,
                u_width: actual_u_width,
                v_height: v_step,
                width: actual_width as u8,
                height: char_height as u8,
            };

            char_table.insert(char_index as u8, font_char);
        }

        // Create texture from surface
        let texture = TextureClass::from_surface(name, surface)
            .map_err(|e| FontError::ImageLoadError(format!("Failed to create texture: {}", e)))?;

        Ok(Font3DData {
            name: name.to_string(),
            texture: Some(texture),
            char_table,
            char_height: char_height as u8,
            space_width: (char_width * 0.5) as u8,
        })
    }

    /// Create font from a horizontal strike layout
    fn create_font_from_strike(
        &self,
        name: &str,
        surface: &SurfaceClass,
    ) -> Result<Font3DData, FontError> {
        let desc = surface.description();
        let width = desc.width as f32;
        let height = desc.height as f32;

        let mut char_table = HashMap::new();
        let mut column = 0;

        // Scan through the strike to find each character
        for char_index in 0..127 {
            // Skip to the next non-transparent column
            while column < width as usize
                && surface.is_transparent_column(column as u32).unwrap_or(true)
            {
                column += 1;
            }
            let start = column;

            // Find the end of this character
            while column < width as usize
                && !surface.is_transparent_column(column as u32).unwrap_or(true)
            {
                column += 1;
            }
            let end = column;

            if end > start {
                let char_width = (end - start) as f32;
                let u_offset = start as f32 / width;
                let u_width = char_width / width;

                let font_char = FontChar {
                    u_offset,
                    v_offset: 0.0,
                    u_width,
                    v_height: 1.0,
                    width: char_width as u8,
                    height: height as u8,
                };

                char_table.insert(char_index as u8, font_char);
            }
        }

        // Create texture from surface
        let texture = TextureClass::from_surface(name, surface)
            .map_err(|e| FontError::ImageLoadError(format!("Failed to create texture: {}", e)))?;

        Ok(Font3DData {
            name: name.to_string(),
            texture: Some(texture),
            char_table,
            char_height: height as u8,
            space_width: (height * 0.5) as u8,
        })
    }

    /// Find the actual bounds of a character in the texture (for proportional spacing)
    fn find_char_bounds(
        &self,
        surface: &SurfaceClass,
        col: usize,
        row: usize,
        char_width: f32,
        _char_height: f32,
    ) -> (f32, f32, f32) {
        let desc = surface.description();
        let cell_width = (desc.width / 16).max(1);
        let cell_height = (desc.height / 16).max(1);
        let u_step = 1.0 / 16.0;

        let src_rect = SurfaceRect::new(
            (col as u32) * cell_width,
            (row as u32) * cell_height,
            cell_width,
            cell_height,
        );

        if let Ok(cell_surface) = SurfaceClass::new(cell_width, cell_height, desc.format) {
            if cell_surface.blit((0, 0), surface, src_rect).is_ok() {
                if let Ok(Some(bounds)) = cell_surface.find_nonzero_alpha_bounds() {
                    let actual_width = bounds.width.max(1) as f32;
                    let actual_u_offset = ((src_rect.x + bounds.x) as f32) / (desc.width as f32);
                    let actual_u_width = (bounds.width as f32) / (desc.width as f32);
                    return (actual_width, actual_u_offset, actual_u_width);
                }
            }
        }

        let u_offset = col as f32 * u_step;
        (char_width, u_offset, u_step)
    }

    /// Calculate the width of a text string using a specific font
    pub fn calculate_text_width(&self, font_name: &str, text: &str, scale: f32) -> f32 {
        if let Some(font) = self.get_font(font_name) {
            let mut width = 0.0;
            for ch in text.chars() {
                width += font.char_width(ch) as f32 * scale;
            }
            width
        } else {
            0.0
        }
    }
}

impl Default for FontSystem {
    fn default() -> Self {
        Self::new()
    }
}
