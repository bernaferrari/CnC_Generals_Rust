/***********************************************************************************************
 ***                            Confidential - Westwood Studios                              ***
 ***********************************************************************************************
 *                                                                                             *
 *                 Project Name : Commando / G 3D Library                                      *
 *                                                                                             *
 *                     $Archive:: /Commando/Code/ww3d2/font3d.h                               $*
 *                                                                                             *
 *                      $Author:: Byon_g                                                      $*
 *                                                                                             *
 *                     $Modtime:: 4/05/01 2:19p                                               $*
 *                                                                                             *
 *                    $Revision:: 4                                                           $*
 *                                                                                             *
 *---------------------------------------------------------------------------------------------*/

// Placeholder imports
use crate::texture_system::{SurfaceClass, TextureClass};
use math_utilities::Vector4;
use std::sync::Arc;

// Placeholder for reference counting
pub struct RefCountClass;

impl RefCountClass {
    pub fn add_ref(&self) {}
    pub fn release_ref(&self) {}
}

// Placeholder types
pub type WCHAR = u16;

// Rectangle structure placeholder
pub struct RectClass {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl RectClass {
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self { left, top, right, bottom }
    }
}

/// Font3DDataClass - 3D Font Data Class
///
/// This class provides an interface to a font texture. Once created and loaded
/// with a font, the object can return texture u v coordinate for any character
/// in the font, as well as the character width for proportional fonts.
pub struct Font3DDataClass {
    pub name: String,
    pub texture: Option<Arc<TextureClass>>,

    // Character metrics tables
    pub char_width_table: [u8; 256],
    pub char_height: u8,

    // UV coordinate tables
    pub u_offset_table: [f32; 256],
    pub v_offset_table: [f32; 256],
    pub u_width_table: [f32; 256],
    pub v_height: f32,

    // Space width (user-settable)
    pub space_width: f32,

    // Reference counting
    pub ref_count: std::sync::atomic::AtomicU32,
}

impl Font3DDataClass {
    /// Create new font data from file
    pub fn new(filename: &str) -> Self {
        let mut font = Self {
            name: filename.to_string(),
            texture: None,
            char_width_table: [8; 256], // Default width
            char_height: 16, // Default height
            u_offset_table: [0.0; 256],
            v_offset_table: [0.0; 256],
            u_width_table: [0.0625; 256], // 1/16th for 16x16 chars
            v_height: 0.0625, // 1/16th for 16x16 chars
            space_width: 8.0,
            ref_count: std::sync::atomic::AtomicU32::new(1),
        };

        // Load font from TGA file and set up character metrics
        font.load_font_texture(filename);
        font.initialize_font_data();

        font
    }

    /// Load font texture from TGA file
    fn load_font_texture(&mut self, filename: &str) {
        // Load and parse font texture file
        // In a full implementation, this would:
        // 1. Load the TGA file
        // 2. Parse the font metrics from the TGA header/comments
        // 3. Create the texture object

        // For now, create a placeholder texture
        // This would be replaced with actual TGA loading code
        self.texture = Some(Arc::new(TextureClass::default()));
    }

    /// Initialize font data tables
    fn initialize_font_data(&mut self) {
        // Set up character width, height, and UV coordinates
        // In a full implementation, this would parse font metrics from the TGA file
        // For now, set up basic ASCII character mapping assuming 16x16 character grid

        for i in 0..256 {
            let char_index = i % 16;
            let row_index = i / 16;

            self.u_offset_table[i] = (char_index as f32) * 0.0625;
            self.v_offset_table[i] = (row_index as f32) * 0.0625;
            self.u_width_table[i] = 0.0625; // Fixed width for now
            self.char_width_table[i] = 8; // Fixed width for now
        }

        self.char_height = 16;
        self.v_height = 0.0625;
    }

    /// Get character width in pixels
    pub fn char_width(&self, ch: WCHAR) -> u8 {
        let index = (ch & 0xFF) as usize;
        self.char_width_table[index]
    }

    /// Get character height in pixels
    pub fn char_height(&self, _ch: WCHAR) -> u8 {
        self.char_height
    }

    /// Get character U offset in normalized texture space
    pub fn char_u_offset(&self, ch: WCHAR) -> f32 {
        let index = (ch & 0xFF) as usize;
        self.u_offset_table[index]
    }

    /// Get character V offset in normalized texture space
    pub fn char_v_offset(&self, ch: WCHAR) -> f32 {
        let index = (ch & 0xFF) as usize;
        self.v_offset_table[index]
    }

    /// Get character U width in normalized texture space
    pub fn char_u_width(&self, ch: WCHAR) -> f32 {
        let index = (ch & 0xFF) as usize;
        self.u_width_table[index]
    }

    /// Get character V height in normalized texture space
    pub fn char_v_height(&self, _ch: WCHAR) -> f32 {
        self.v_height
    }

    /// Get space width
    pub fn get_space_width(&self) -> f32 {
        self.space_width
    }

    /// Set space width
    pub fn set_space_width(&mut self, width: f32) {
        self.space_width = width;
    }

    /// Get character UV coordinates
    pub fn get_char_uv(&self, ch: WCHAR) -> (f32, f32, f32, f32) {
        let u_offset = self.char_u_offset(ch);
        let v_offset = self.char_v_offset(ch);
        let u_width = self.char_u_width(ch);
        let v_height = self.char_v_height(ch);

        (u_offset, v_offset, u_offset + u_width, v_offset + v_height)
    }

    /// Get font texture
    pub fn get_texture(&self) -> Option<&Arc<TextureClass>> {
        self.texture.as_ref()
    }

    /// Set font texture
    pub fn set_texture(&mut self, texture: Arc<TextureClass>) {
        self.texture = Some(texture);
    }

    /// Get font name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Load font from file
    pub fn load(&mut self, filename: &str) -> bool {
        self.name = filename.to_string();

        // Load and parse font texture file
        self.load_font_texture(filename);
        self.initialize_font_data();

        self.texture.is_some()
    }

    /// Save font to file
    pub fn save(&self, filename: &str) -> bool {
        // Save font data to file
        // In a full implementation, this would save the font metrics and texture
        // For now, return true to indicate success
        let _ = filename; // Use the filename parameter
        true
    }

    /// Free font resources
    pub fn free(&mut self) {
        self.texture = None;
        // Free other resources (character tables, etc.)
        // All resources are automatically freed when the object is dropped
    }

    /// Is font loaded?
    pub fn is_loaded(&self) -> bool {
        self.texture.is_some()
    }
}

impl Drop for Font3DDataClass {
    fn drop(&mut self) {
        self.free();
    }
}

/// Font3DInstanceClass - 3D Font Instance Class
///
/// This class represents an instance of a 3D font that can be used for rendering text.
pub struct Font3DInstanceClass {
    pub font_data: Option<Arc<Font3DDataClass>>,
    pub color: Vector4,
    pub position: Vec3,
    pub scale: f32,
}

impl Font3DInstanceClass {
    /// Create new font instance
    pub fn new() -> Self {
        Self {
            font_data: None,
            color: Vector4::new(1.0, 1.0, 1.0, 1.0),
            position: Vec3::new(0.0, 0.0, 0.0),
            scale: 1.0,
        }
    }

    /// Create font instance with font data
    pub fn with_font_data(font_data: Arc<Font3DDataClass>) -> Self {
        Self {
            font_data: Some(font_data),
            color: Vector4::new(1.0, 1.0, 1.0, 1.0),
            position: Vec3::new(0.0, 0.0, 0.0),
            scale: 1.0,
        }
    }

    /// Set font data
    pub fn set_font_data(&mut self, font_data: Arc<Font3DDataClass>) {
        self.font_data = Some(font_data);
    }

    /// Get font data
    pub fn get_font_data(&self) -> Option<&Arc<Font3DDataClass>> {
        self.font_data.as_ref()
    }

    /// Set color
    pub fn set_color(&mut self, color: Vector4) {
        self.color = color;
    }

    /// Get color
    pub fn get_color(&self) -> &Vector4 {
        &self.color
    }

    /// Set position
    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
    }

    /// Get position
    pub fn get_position(&self) -> &Vec3 {
        &self.position
    }

    /// Set scale
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
    }

    /// Get scale
    pub fn get_scale(&self) -> f32 {
        self.scale
    }

    /// Render text
    pub fn render_text(&self, text: &str) {
        if self.font_data.is_none() {
            return;
        }

        let font_data = self.font_data.as_ref().unwrap();
        let mut current_x = self.position.x;
        let current_y = self.position.y;
        let current_z = self.position.z;

        // Iterate through characters and render quads for each one
        for ch in text.chars() {
            if ch == ' ' {
                current_x += font_data.get_space_width() * self.scale;
                continue;
            }

            if ch == '\n' {
                current_x = self.position.x;
                // current_y -= font_data.char_height as f32 * self.scale; // Uncomment for multi-line support
                continue;
            }

            let char_index = (ch as WCHAR & 0xFF) as usize;

            // Get character metrics
            let char_width = font_data.char_width_table[char_index] as f32 * self.scale;
            let char_height = font_data.char_height as f32 * self.scale;

            // Get UV coordinates
            let u_offset = font_data.u_offset_table[char_index];
            let v_offset = font_data.v_offset_table[char_index];
            let u_width = font_data.u_width_table[char_index];
            let v_height = font_data.v_height;

            // In a full implementation, this would:
            // 1. Create a quad with the character UV coordinates
            // 2. Set up the vertex positions and colors
            // 3. Submit to the renderer

            // For now, just advance the position
            current_x += char_width;
        }
    }

    /// Get text width
    pub fn get_text_width(&self, text: &str) -> f32 {
        if self.font_data.is_none() {
            return 0.0;
        }

        let mut width = 0.0;
        for ch in text.chars() {
            if ch == ' ' {
                if let Some(font) = &self.font_data {
                    width += font.get_space_width();
                }
            } else {
                if let Some(font) = &self.font_data {
                    width += font.char_width(ch as WCHAR) as f32;
                }
            }
        }

        width * self.scale
    }

    /// Get text height
    pub fn get_text_height(&self, _text: &str) -> f32 {
        if let Some(font) = &self.font_data {
            font.char_height as f32 * self.scale
        } else {
            0.0
        }
    }

    /// Set monospace mode
    pub fn set_monospace(&mut self, monospace: bool) {
        // Set monospace mode for the font
        // In monospace mode, all characters have the same width
        if let Some(font_data) = &mut self.font_data {
            if monospace {
                // Set all characters to the same width (use space width)
                let mono_width = font_data.space_width as u8;
                for i in 0..256 {
                    font_data.char_width_table[i] = mono_width;
                }
            } else {
                // Restore original character widths
                // This would reload the original metrics from the font file
                font_data.initialize_font_data();
            }
        }
    }

    /// Is monospace?
    pub fn is_monospace(&self) -> bool {
        // Check if all characters have the same width
        if let Some(font_data) = &self.font_data {
            let first_width = font_data.char_width_table[0];
            font_data.char_width_table.iter().all(|&width| width == first_width)
        } else {
            false
        }
    }
}

impl Default for Font3DInstanceClass {
    fn default() -> Self {
        Self::new()
    }
}
