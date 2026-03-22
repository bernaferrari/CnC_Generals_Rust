//! INI Control Bar Scheme parsing module
//! Created: Apr 2002
//! Filename: INIControlBarScheme.cpp
//! Author: Chris Huybregts
//! Purpose: Parse a control Bar Scheme

use super::ini::INILoadType;
use super::ini::{FieldParse, INIError, INIResult, INI};
use super::ini_mapped_image::ICoord2D;
use log::warn;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Maximum control bar scheme image layers (matches C++ MAX_CONTROL_BAR_SCHEME_IMAGE_LAYERS)
const MAX_CONTROL_BAR_SCHEME_IMAGE_LAYERS: usize = 6;
/// Foreground image layer boundary (layers 0-2 are foreground, 3-5 are background)
const CONTROL_BAR_SCHEME_FOREGROUND_IMAGE_LAYERS: usize = 3;

/// Represents a single image entry within a control bar scheme layer
#[derive(Debug, Clone)]
pub struct SchemeImage {
    /// Name of the image (resolved via mapped image collection at draw time)
    pub image_name: String,
    /// Draw position offset
    pub position: ICoord2D,
    /// Draw size
    pub size: ICoord2D,
    /// Layer index (0-5, 0 = top/foreground, 5 = bottom/background)
    pub layer: i32,
}

impl Default for SchemeImage {
    fn default() -> Self {
        Self {
            image_name: String::new(),
            position: ICoord2D { x: 0, y: 0 },
            size: ICoord2D { x: 0, y: 0 },
            layer: 0,
        }
    }
}

/// Parsed `ImagePart` content from a control bar scheme.
#[derive(Debug, Clone, Default)]
pub struct ControlBarSchemeImagePart {
    pub image: SchemeImage,
}

/// Parsed `AnimatingPart` content from a control bar scheme.
#[derive(Debug, Clone, Default)]
pub struct ControlBarSchemeAnimationPart {
    pub name: String,
    pub animation_name: String,
    pub duration_ms: u32,
    pub final_pos: ICoord2D,
    pub image: SchemeImage,
}

/// Parsed `VideoPart` content from a control bar scheme.
#[derive(Debug, Clone, Default)]
pub struct ControlBarSchemeVideoPart {
    pub name: String,
    pub image: SchemeImage,
    pub properties: HashMap<String, String>,
}

/// Draw function callback type: (image_name, start_x, start_y, end_x, end_y) -> ()
pub type SchemeDrawFunc = fn(&str, i32, i32, i32, i32);

/// Global draw function for scheme images (set once from the rendering layer)
static SCHEME_DRAW_FUNC: OnceCell<SchemeDrawFunc> = OnceCell::new();

/// Set the global draw callback for scheme image rendering.
/// Must be called once before any scheme drawing occurs.
pub fn set_scheme_draw_func(func: SchemeDrawFunc) {
    let _ = SCHEME_DRAW_FUNC.set(func);
}

/// Control bar scheme structure for UI layout configuration
#[derive(Debug, Clone)]
pub struct ControlBarScheme {
    pub name: String,
    pub screen_width: u32,
    pub screen_height: u32,
    pub side_panel_width: u32,
    pub side_panel_height: u32,
    pub button_panel_width: u32,
    pub button_panel_height: u32,
    pub command_bar_width: u32,
    pub command_bar_height: u32,
    pub control_bar_x: i32,
    pub control_bar_y: i32,
    pub control_bar_width: u32,
    pub control_bar_height: u32,
    pub minimap_x: i32,
    pub minimap_y: i32,
    pub minimap_width: u32,
    pub minimap_height: u32,
    pub power_bar_x: i32,
    pub power_bar_y: i32,
    pub power_bar_width: u32,
    pub power_bar_height: u32,
    pub money_display_x: i32,
    pub money_display_y: i32,
    pub money_display_width: u32,
    pub money_display_height: u32,
    pub general_exp_bar_x: i32,
    pub general_exp_bar_y: i32,
    pub general_exp_bar_width: u32,
    pub general_exp_bar_height: u32,
    pub chat_panel_x: i32,
    pub chat_panel_y: i32,
    pub chat_panel_width: u32,
    pub chat_panel_height: u32,
    pub beacon_panel_x: i32,
    pub beacon_panel_y: i32,
    pub beacon_panel_width: u32,
    pub beacon_panel_height: u32,
    pub popup_message_x: i32,
    pub popup_message_y: i32,
    pub popup_message_width: u32,
    pub popup_message_height: u32,
    pub background_image: String,
    pub button_layout: String,
    pub font_name: String,
    pub font_size: u32,
    pub text_color_r: f32,
    pub text_color_g: f32,
    pub text_color_b: f32,
    pub text_color_a: f32,
    pub border_color_r: f32,
    pub border_color_g: f32,
    pub border_color_b: f32,
    pub border_color_a: f32,
    pub highlight_color_r: f32,
    pub highlight_color_g: f32,
    pub highlight_color_b: f32,
    pub highlight_color_a: f32,
    /// Image layers (6 layers total, 0-2 foreground, 3-5 background)
    pub scheme_images: Vec<SchemeImage>,
    /// Parsed `ImagePart` entries, preserved for parity and inspection.
    pub image_parts: Vec<ControlBarSchemeImagePart>,
    /// Parsed `AnimatingPart` entries, preserved for parity and inspection.
    pub animation_parts: Vec<ControlBarSchemeAnimationPart>,
    /// Parsed `VideoPart` entries, preserved for parity and inspection.
    pub video_parts: Vec<ControlBarSchemeVideoPart>,
    /// Screen creation resolution (matches C++ m_ScreenCreationRes)
    pub screen_creation_res: ICoord2D,
}

impl Default for ControlBarScheme {
    fn default() -> Self {
        Self {
            name: String::new(),
            screen_width: 1024,
            screen_height: 768,
            side_panel_width: 256,
            side_panel_height: 768,
            button_panel_width: 200,
            button_panel_height: 150,
            command_bar_width: 768,
            command_bar_height: 200,
            control_bar_x: 0,
            control_bar_y: 568,
            control_bar_width: 1024,
            control_bar_height: 200,
            minimap_x: 10,
            minimap_y: 578,
            minimap_width: 160,
            minimap_height: 120,
            power_bar_x: 180,
            power_bar_y: 700,
            power_bar_width: 64,
            power_bar_height: 64,
            money_display_x: 250,
            money_display_y: 578,
            money_display_width: 100,
            money_display_height: 20,
            general_exp_bar_x: 360,
            general_exp_bar_y: 578,
            general_exp_bar_width: 100,
            general_exp_bar_height: 16,
            chat_panel_x: 10,
            chat_panel_y: 10,
            chat_panel_width: 400,
            chat_panel_height: 100,
            beacon_panel_x: 500,
            beacon_panel_y: 10,
            beacon_panel_width: 200,
            beacon_panel_height: 50,
            popup_message_x: 200,
            popup_message_y: 200,
            popup_message_width: 400,
            popup_message_height: 100,
            background_image: String::new(),
            button_layout: "Grid3x6".to_string(),
            font_name: "Arial".to_string(),
            font_size: 12,
            text_color_r: 1.0,
            text_color_g: 1.0,
            text_color_b: 1.0,
            text_color_a: 1.0,
            border_color_r: 0.5,
            border_color_g: 0.5,
            border_color_b: 0.5,
            border_color_a: 1.0,
            highlight_color_r: 1.0,
            highlight_color_g: 1.0,
            highlight_color_b: 0.0,
            highlight_color_a: 1.0,
            scheme_images: Vec::new(),
            image_parts: Vec::new(),
            animation_parts: Vec::new(),
            video_parts: Vec::new(),
            screen_creation_res: ICoord2D { x: 800, y: 600 },
        }
    }
}

impl ControlBarScheme {
    /// Create a new control bar scheme with the given name
    pub fn new(name: String) -> Self {
        Self {
            name: name.trim().to_lowercase(),
            ..Default::default()
        }
    }

    /// Get the name of this control bar scheme
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Parse control bar scheme from INI
    pub fn parse_from_ini(ini: &mut INI, name: String) -> INIResult<Self> {
        let mut scheme = Self::new(name);
        scheme.parse_scheme_fields(ini)?;
        Ok(scheme)
    }

    /// Parse control bar scheme fields
    fn parse_scheme_fields(&mut self, ini: &mut INI) -> INIResult<()> {
        loop {
            ini.read_line()?;
            if ini.is_end_of_file() {
                return Err(INIError::EndOfFile);
            }

            let line = ini.get_buffer().to_string();
            let mut parts = line.split_whitespace();
            let Some(key) = parts.next() else {
                continue;
            };

            if key.eq_ignore_ascii_case("End") {
                break;
            }

            if key.eq_ignore_ascii_case("ImagePart") {
                let image_part = Self::parse_image_part_block(ini)?;
                self.scheme_images.push(image_part.image.clone());
                self.image_parts.push(image_part);
                continue;
            }

            if key.eq_ignore_ascii_case("AnimatingPart") {
                let animation_part = Self::parse_animating_part_block(ini)?;
                self.scheme_images.push(animation_part.image.clone());
                self.animation_parts.push(animation_part);
                continue;
            }

            if key.eq_ignore_ascii_case("VideoPart") {
                let video_part = Self::parse_video_part_block(ini)?;
                if !video_part.image.image_name.is_empty() {
                    self.scheme_images.push(video_part.image.clone());
                }
                self.video_parts.push(video_part);
                continue;
            }

            let mut value_tokens: Vec<&str> = parts.collect();
            value_tokens.retain(|token| *token != "=");

            for field in Self::get_field_parse() {
                if field.token.eq_ignore_ascii_case(key) {
                    (field.parse)(ini, self, &value_tokens)?;
                    break;
                }
            }
        }

        Ok(())
    }

    fn parse_image_part_block(ini: &mut INI) -> INIResult<ControlBarSchemeImagePart> {
        let mut image_part = ControlBarSchemeImagePart::default();

        loop {
            ini.read_line()?;
            if ini.is_end_of_file() {
                return Err(INIError::EndOfFile);
            }

            let line = ini.get_buffer().to_string();
            let mut parts = line.split_whitespace();
            let Some(key) = parts.next() else {
                continue;
            };

            if key.eq_ignore_ascii_case("End") {
                break;
            }

            let mut value_tokens: Vec<&str> = parts.collect();
            value_tokens.retain(|token| *token != "=");
            if value_tokens.is_empty() {
                continue;
            }

            match key.to_ascii_lowercase().as_str() {
                "position" => {
                    let (x, y) = parse_icoord2d(&value_tokens)?;
                    image_part.image.position = ICoord2D { x, y };
                }
                "size" => {
                    let (x, y) = parse_icoord2d(&value_tokens)?;
                    image_part.image.size = ICoord2D { x, y };
                }
                "imagename" => {
                    image_part.image.image_name = value_tokens[0].to_string();
                }
                "layer" => {
                    image_part.image.layer = INI::parse_int(value_tokens[0])?;
                }
                _ => {}
            }
        }

        Ok(image_part)
    }

    fn parse_animating_part_block(ini: &mut INI) -> INIResult<ControlBarSchemeAnimationPart> {
        let mut animation_part = ControlBarSchemeAnimationPart::default();

        loop {
            ini.read_line()?;
            if ini.is_end_of_file() {
                return Err(INIError::EndOfFile);
            }

            let line = ini.get_buffer().to_string();
            let mut parts = line.split_whitespace();
            let Some(key) = parts.next() else {
                continue;
            };

            if key.eq_ignore_ascii_case("End") {
                break;
            }

            if key.eq_ignore_ascii_case("ImagePart") {
                animation_part.image = Self::parse_scheme_image_block(ini)?;
                continue;
            }

            let mut value_tokens: Vec<&str> = parts.collect();
            value_tokens.retain(|token| *token != "=");
            if value_tokens.is_empty() {
                continue;
            }

            match key.to_ascii_lowercase().as_str() {
                "name" => animation_part.name = value_tokens[0].to_string(),
                "animation" => animation_part.animation_name = value_tokens[0].to_string(),
                "duration" => {
                    animation_part.duration_ms = INI::parse_unsigned_int(value_tokens[0])?
                }
                "finalpos" => {
                    let (x, y) = parse_icoord2d(&value_tokens)?;
                    animation_part.final_pos = ICoord2D { x, y };
                }
                _ => {}
            }
        }

        Ok(animation_part)
    }

    fn parse_video_part_block(ini: &mut INI) -> INIResult<ControlBarSchemeVideoPart> {
        let mut video_part = ControlBarSchemeVideoPart::default();

        loop {
            ini.read_line()?;
            if ini.is_end_of_file() {
                return Err(INIError::EndOfFile);
            }

            let line = ini.get_buffer().to_string();
            let mut parts = line.split_whitespace();
            let Some(key) = parts.next() else {
                continue;
            };

            if key.eq_ignore_ascii_case("End") {
                break;
            }

            if key.eq_ignore_ascii_case("ImagePart") {
                let image = Self::parse_scheme_image_block(ini)?;
                video_part.image = image;
                continue;
            }

            let mut value_tokens: Vec<&str> = parts.collect();
            value_tokens.retain(|token| *token != "=");
            if value_tokens.is_empty() {
                continue;
            }

            let value = value_tokens.join(" ");
            if key.eq_ignore_ascii_case("Name") {
                video_part.name = value.clone();
            }
            video_part.properties.insert(key.to_string(), value);
        }

        Ok(video_part)
    }

    fn parse_scheme_image_block(ini: &mut INI) -> INIResult<SchemeImage> {
        let mut image = SchemeImage::default();

        loop {
            ini.read_line()?;
            if ini.is_end_of_file() {
                return Err(INIError::EndOfFile);
            }

            let line = ini.get_buffer().to_string();
            let mut parts = line.split_whitespace();
            let Some(key) = parts.next() else {
                continue;
            };

            if key.eq_ignore_ascii_case("End") {
                break;
            }

            let mut value_tokens: Vec<&str> = parts.collect();
            value_tokens.retain(|token| *token != "=");
            if value_tokens.is_empty() {
                continue;
            }

            match key.to_ascii_lowercase().as_str() {
                "position" => {
                    let (x, y) = parse_icoord2d(&value_tokens)?;
                    image.position = ICoord2D { x, y };
                }
                "size" => {
                    let (x, y) = parse_icoord2d(&value_tokens)?;
                    image.size = ICoord2D { x, y };
                }
                "imagename" => {
                    image.image_name = value_tokens[0].to_string();
                }
                "layer" => {
                    image.layer = INI::parse_int(value_tokens[0])?;
                }
                _ => {}
            }
        }

        Ok(image)
    }

    /// Get the field parsing table for control bar schemes
    pub fn get_field_parse() -> &'static [FieldParse<Self>] {
        CONTROL_BAR_SCHEME_FIELDS
    }

    /// Check if this scheme supports the given screen resolution
    pub fn supports_resolution(&self, width: u32, height: u32) -> bool {
        self.screen_width == width && self.screen_height == height
    }

    /// Get the aspect ratio of this scheme
    pub fn get_aspect_ratio(&self) -> f32 {
        self.screen_width as f32 / self.screen_height as f32
    }

    /// Check if coordinates are within the control bar area
    pub fn is_in_control_bar(&self, x: i32, y: i32) -> bool {
        x >= self.control_bar_x
            && x < (self.control_bar_x + self.control_bar_width as i32)
            && y >= self.control_bar_y
            && y < (self.control_bar_y + self.control_bar_height as i32)
    }

    /// Check if coordinates are within the minimap area
    pub fn is_in_minimap(&self, x: i32, y: i32) -> bool {
        x >= self.minimap_x
            && x < (self.minimap_x + self.minimap_width as i32)
            && y >= self.minimap_y
            && y < (self.minimap_y + self.minimap_height as i32)
    }

    /// Get text color as RGBA tuple
    pub fn get_text_color(&self) -> (f32, f32, f32, f32) {
        (
            self.text_color_r,
            self.text_color_g,
            self.text_color_b,
            self.text_color_a,
        )
    }

    /// Get border color as RGBA tuple
    pub fn get_border_color(&self) -> (f32, f32, f32, f32) {
        (
            self.border_color_r,
            self.border_color_g,
            self.border_color_b,
            self.border_color_a,
        )
    }

    /// Get highlight color as RGBA tuple
    pub fn get_highlight_color(&self) -> (f32, f32, f32, f32) {
        (
            self.highlight_color_r,
            self.highlight_color_g,
            self.highlight_color_b,
            self.highlight_color_a,
        )
    }

    /// Set text color from RGBA tuple
    pub fn set_text_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.text_color_r = r;
        self.text_color_g = g;
        self.text_color_b = b;
        self.text_color_a = a;
    }

    /// Set border color from RGBA tuple
    pub fn set_border_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.border_color_r = r;
        self.border_color_g = g;
        self.border_color_b = b;
        self.border_color_a = a;
    }

    /// Set highlight color from RGBA tuple
    pub fn set_highlight_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.highlight_color_r = r;
        self.highlight_color_g = g;
        self.highlight_color_b = b;
        self.highlight_color_a = a;
    }

    /// Validate the control bar scheme configuration
    pub fn validate(&self) -> INIResult<()> {
        // Check for reasonable screen dimensions
        if self.screen_width == 0 || self.screen_height == 0 {
            eprintln!(
                "ControlBarScheme {} has invalid screen dimensions",
                self.name
            );
            return Err(INIError::InvalidData);
        }

        // Check that UI elements fit within screen bounds
        if (self.control_bar_x + self.control_bar_width as i32) > self.screen_width as i32 {
            eprintln!(
                "ControlBarScheme {} control bar extends beyond screen width",
                self.name
            );
            return Err(INIError::InvalidData);
        }

        if (self.control_bar_y + self.control_bar_height as i32) > self.screen_height as i32 {
            eprintln!(
                "ControlBarScheme {} control bar extends beyond screen height",
                self.name
            );
            return Err(INIError::InvalidData);
        }

        // Validate color values (should be 0.0 to 1.0)
        let colors = [
            (self.text_color_r, "text_color_r"),
            (self.text_color_g, "text_color_g"),
            (self.text_color_b, "text_color_b"),
            (self.text_color_a, "text_color_a"),
            (self.border_color_r, "border_color_r"),
            (self.border_color_g, "border_color_g"),
            (self.border_color_b, "border_color_b"),
            (self.border_color_a, "border_color_a"),
            (self.highlight_color_r, "highlight_color_r"),
            (self.highlight_color_g, "highlight_color_g"),
            (self.highlight_color_b, "highlight_color_b"),
            (self.highlight_color_a, "highlight_color_a"),
        ];

        for (value, name) in colors.iter() {
            if *value < 0.0 || *value > 1.0 {
                eprintln!(
                    "ControlBarScheme {} has invalid color value for {}: {}",
                    self.name, name, value
                );
                return Err(INIError::InvalidData);
            }
        }

        Ok(())
    }
}

/// Control bar scheme manager for handling collections of control bar schemes
#[derive(Debug)]
pub struct ControlBarSchemeManager {
    schemes: HashMap<String, ControlBarScheme>,
    active_scheme: Option<String>,
    background_marker_pos: ICoord2D,
    foreground_marker_pos: ICoord2D,
    /// Multiplier for scheme image positions (screen_size / creation_res)
    multiplier_x: f32,
    multiplier_y: f32,
}

impl ControlBarSchemeManager {
    /// Create a new control bar scheme manager
    pub fn new() -> Self {
        Self {
            schemes: HashMap::new(),
            active_scheme: None,
            background_marker_pos: ICoord2D { x: 0, y: 0 },
            foreground_marker_pos: ICoord2D { x: 0, y: 0 },
            multiplier_x: 1.0,
            multiplier_y: 1.0,
        }
    }

    /// Set the background marker position (called from INI parsing)
    pub fn set_background_marker_pos(&mut self, x: i32, y: i32) {
        self.background_marker_pos = ICoord2D { x, y };
    }

    /// Set the foreground marker position (called from INI parsing)
    pub fn set_foreground_marker_pos(&mut self, x: i32, y: i32) {
        self.foreground_marker_pos = ICoord2D { x, y };
    }

    /// Get the background marker base position
    pub fn get_background_marker_pos(&self) -> ICoord2D {
        self.background_marker_pos
    }

    /// Get the foreground marker base position
    pub fn get_foreground_marker_pos(&self) -> ICoord2D {
        self.foreground_marker_pos
    }

    /// Draw the background layers at the given offset
    /// Layers 3-5 are background (matches C++ ControlBarScheme::drawBackground)
    pub fn draw_background(&self, offset: ICoord2D) {
        let Some(scheme) = self.get_active_scheme() else {
            return;
        };
        let Some(draw) = SCHEME_DRAW_FUNC.get() else {
            return;
        };
        // Background layers: iterate layers 5 down to 3 (matches C++ loop)
        for layer_idx in (CONTROL_BAR_SCHEME_FOREGROUND_IMAGE_LAYERS as i32
            ..MAX_CONTROL_BAR_SCHEME_IMAGE_LAYERS as i32)
            .rev()
        {
            for image in &scheme.scheme_images {
                if image.layer != layer_idx {
                    continue;
                }
                if image.image_name.is_empty() {
                    continue;
                }
                let sx = (image.position.x as f32 * self.multiplier_x) as i32 + offset.x;
                let sy = (image.position.y as f32 * self.multiplier_y) as i32 + offset.y;
                let ex = ((image.position.x + image.size.x) as f32 * self.multiplier_x) as i32
                    + offset.x;
                let ey = ((image.position.y + image.size.y) as f32 * self.multiplier_y) as i32
                    + offset.y;
                (draw)(&image.image_name, sx, sy, ex, ey);
            }
        }
    }

    /// Draw the foreground layers at the given offset
    /// Layers 0-2 are foreground (matches C++ ControlBarScheme::drawForeground)
    pub fn draw_foreground(&self, offset: ICoord2D) {
        let Some(scheme) = self.get_active_scheme() else {
            return;
        };
        let Some(draw) = SCHEME_DRAW_FUNC.get() else {
            return;
        };
        // Foreground layers: iterate layers 2 down to 0 (matches C++ loop)
        for layer_idx in (0..CONTROL_BAR_SCHEME_FOREGROUND_IMAGE_LAYERS as i32).rev() {
            for image in &scheme.scheme_images {
                if image.layer != layer_idx {
                    continue;
                }
                if image.image_name.is_empty() {
                    continue;
                }
                let sx = (image.position.x as f32 * self.multiplier_x) as i32 + offset.x;
                let sy = (image.position.y as f32 * self.multiplier_y) as i32 + offset.y;
                let ex = ((image.position.x + image.size.x) as f32 * self.multiplier_x) as i32
                    + offset.x;
                let ey = ((image.position.y + image.size.y) as f32 * self.multiplier_y) as i32
                    + offset.y;
                (draw)(&image.image_name, sx, sy, ex, ey);
            }
        }
    }

    /// Add a scheme image to the active scheme
    pub fn add_scheme_image(&mut self, image: SchemeImage) {
        if let Some(scheme) = self.get_active_scheme_mut() {
            scheme.scheme_images.push(image);
        }
    }

    /// Set the position/size multiplier (screen_size / creation_res)
    pub fn set_multiplier(&mut self, x: f32, y: f32) {
        self.multiplier_x = x;
        self.multiplier_y = y;
    }

    /// Get the current multiplier
    pub fn get_multiplier(&self) -> (f32, f32) {
        (self.multiplier_x, self.multiplier_y)
    }

    /// Find a control bar scheme by name
    pub fn find_scheme(&self, name: &str) -> Option<&ControlBarScheme> {
        self.schemes.get(&name.trim().to_lowercase())
    }

    /// Find a mutable control bar scheme by name
    pub fn find_scheme_mut(&mut self, name: &str) -> Option<&mut ControlBarScheme> {
        self.schemes.get_mut(&name.trim().to_lowercase())
    }

    /// Create a new control bar scheme (or return existing cleared one)
    pub fn new_control_bar_scheme(&mut self, name: String) -> &mut ControlBarScheme {
        let normalized_name = name.trim().to_lowercase();
        let scheme = ControlBarScheme::new(normalized_name.clone());
        self.schemes.insert(normalized_name.clone(), scheme);
        self.schemes.get_mut(&normalized_name).unwrap()
    }

    /// Remove a control bar scheme
    pub fn remove_scheme(&mut self, name: &str) -> Option<ControlBarScheme> {
        let normalized_name = name.trim().to_lowercase();
        // If removing the active scheme, clear the active reference
        if let Some(ref active_name) = self.active_scheme {
            if active_name == &normalized_name {
                self.active_scheme = None;
            }
        }
        self.schemes.remove(&normalized_name)
    }

    /// Set the active control bar scheme
    pub fn set_active_scheme(&mut self, name: String) -> Result<(), &'static str> {
        let normalized_name = name.trim().to_lowercase();
        if self.schemes.contains_key(&normalized_name) {
            self.active_scheme = Some(normalized_name);
            Ok(())
        } else {
            self.active_scheme = None;
            Err("Scheme not found")
        }
    }

    /// Get the active control bar scheme
    pub fn get_active_scheme(&self) -> Option<&ControlBarScheme> {
        self.active_scheme
            .as_ref()
            .and_then(|name| self.schemes.get(name))
    }

    /// Get the active control bar scheme (mutable)
    pub fn get_active_scheme_mut(&mut self) -> Option<&mut ControlBarScheme> {
        let active_name = self.active_scheme.clone()?;
        self.schemes.get_mut(&active_name)
    }

    /// Get all scheme names
    pub fn get_scheme_names(&self) -> Vec<&String> {
        self.schemes.keys().collect()
    }

    /// Get the number of schemes
    pub fn count(&self) -> usize {
        self.schemes.len()
    }

    /// Clear all schemes
    pub fn clear(&mut self) {
        self.schemes.clear();
        self.active_scheme = None;
    }

    /// Find the best scheme for a given resolution
    pub fn find_scheme_for_resolution(&self, width: u32, height: u32) -> Option<&ControlBarScheme> {
        // First try to find exact match
        for scheme in self.schemes.values() {
            if scheme.supports_resolution(width, height) {
                return Some(scheme);
            }
        }

        // If no exact match, find closest aspect ratio
        let target_aspect = width as f32 / height as f32;
        let mut best_scheme = None;
        let mut best_diff = f32::INFINITY;

        for scheme in self.schemes.values() {
            let scheme_aspect = scheme.get_aspect_ratio();
            let diff = (scheme_aspect - target_aspect).abs();
            if diff < best_diff {
                best_diff = diff;
                best_scheme = Some(scheme);
            }
        }

        best_scheme
    }

    /// Get the field parsing table for control bar scheme manager
    pub fn get_field_parse() -> &'static [FieldParse<ControlBarScheme>] {
        CONTROL_BAR_SCHEME_FIELDS
    }
}

fn parse_u32_token(args: &[&str]) -> INIResult<u32> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    INI::parse_unsigned_int(token)
}

fn parse_i32_token(args: &[&str]) -> INIResult<i32> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    INI::parse_int(token)
}

fn parse_string_token(args: &[&str]) -> INIResult<String> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    INI::parse_ascii_string(token)
}

fn parse_icoord2d(args: &[&str]) -> INIResult<(i32, i32)> {
    let mut x = 0i32;
    let mut y = 0i32;

    let mut i = 0usize;
    while i < args.len() {
        let token = args[i];
        if let Some(rest) = token.strip_prefix("X:") {
            let value_str = if rest.is_empty() && i + 1 < args.len() {
                i += 1;
                args[i]
            } else {
                rest
            };
            x = INI::parse_int(value_str)?;
        } else if let Some(rest) = token.strip_prefix("Y:") {
            let value_str = if rest.is_empty() && i + 1 < args.len() {
                i += 1;
                args[i]
            } else {
                rest
            };
            y = INI::parse_int(value_str)?;
        }
        i += 1;
    }

    Ok((x, y))
}

fn parse_rgba_tokens(args: &[&str]) -> INIResult<(f32, f32, f32, f32)> {
    if args.len() >= 4 {
        return Ok((
            INI::parse_real(args[0])?,
            INI::parse_real(args[1])?,
            INI::parse_real(args[2])?,
            INI::parse_real(args[3])?,
        ));
    }

    // Allow packed ARGB/RGBA integer values for compatibility.
    if args.len() == 1 {
        let packed = INI::parse_unsigned_int(args[0])?;
        let a = ((packed >> 24) & 0xff) as f32 / 255.0;
        let r = ((packed >> 16) & 0xff) as f32 / 255.0;
        let g = ((packed >> 8) & 0xff) as f32 / 255.0;
        let b = (packed & 0xff) as f32 / 255.0;
        return Ok((r, g, b, a));
    }

    Err(INIError::InvalidData)
}

macro_rules! parse_u32_field {
    ($fn_name:ident, $field:ident) => {
        fn $fn_name(_ini: &mut INI, scheme: &mut ControlBarScheme, args: &[&str]) -> INIResult<()> {
            scheme.$field = parse_u32_token(args)?;
            Ok(())
        }
    };
}

macro_rules! parse_i32_field {
    ($fn_name:ident, $field:ident) => {
        fn $fn_name(_ini: &mut INI, scheme: &mut ControlBarScheme, args: &[&str]) -> INIResult<()> {
            scheme.$field = parse_i32_token(args)?;
            Ok(())
        }
    };
}

macro_rules! parse_string_field {
    ($fn_name:ident, $field:ident) => {
        fn $fn_name(_ini: &mut INI, scheme: &mut ControlBarScheme, args: &[&str]) -> INIResult<()> {
            scheme.$field = parse_string_token(args)?;
            Ok(())
        }
    };
}

parse_u32_field!(parse_screen_width, screen_width);
parse_u32_field!(parse_screen_height, screen_height);
parse_u32_field!(parse_side_panel_width, side_panel_width);
parse_u32_field!(parse_side_panel_height, side_panel_height);
parse_u32_field!(parse_button_panel_width, button_panel_width);
parse_u32_field!(parse_button_panel_height, button_panel_height);
parse_u32_field!(parse_command_bar_width, command_bar_width);
parse_u32_field!(parse_command_bar_height, command_bar_height);
parse_i32_field!(parse_control_bar_x, control_bar_x);
parse_i32_field!(parse_control_bar_y, control_bar_y);
parse_u32_field!(parse_control_bar_width, control_bar_width);
parse_u32_field!(parse_control_bar_height, control_bar_height);
parse_i32_field!(parse_minimap_x, minimap_x);
parse_i32_field!(parse_minimap_y, minimap_y);
parse_u32_field!(parse_minimap_width, minimap_width);
parse_u32_field!(parse_minimap_height, minimap_height);
parse_i32_field!(parse_power_bar_x, power_bar_x);
parse_i32_field!(parse_power_bar_y, power_bar_y);
parse_u32_field!(parse_power_bar_width, power_bar_width);
parse_u32_field!(parse_power_bar_height, power_bar_height);
parse_i32_field!(parse_money_display_x, money_display_x);
parse_i32_field!(parse_money_display_y, money_display_y);
parse_u32_field!(parse_money_display_width, money_display_width);
parse_u32_field!(parse_money_display_height, money_display_height);
parse_i32_field!(parse_general_exp_bar_x, general_exp_bar_x);
parse_i32_field!(parse_general_exp_bar_y, general_exp_bar_y);
parse_u32_field!(parse_general_exp_bar_width, general_exp_bar_width);
parse_u32_field!(parse_general_exp_bar_height, general_exp_bar_height);
parse_i32_field!(parse_chat_panel_x, chat_panel_x);
parse_i32_field!(parse_chat_panel_y, chat_panel_y);
parse_u32_field!(parse_chat_panel_width, chat_panel_width);
parse_u32_field!(parse_chat_panel_height, chat_panel_height);
parse_i32_field!(parse_beacon_panel_x, beacon_panel_x);
parse_i32_field!(parse_beacon_panel_y, beacon_panel_y);
parse_u32_field!(parse_beacon_panel_width, beacon_panel_width);
parse_u32_field!(parse_beacon_panel_height, beacon_panel_height);
parse_i32_field!(parse_popup_message_x, popup_message_x);
parse_i32_field!(parse_popup_message_y, popup_message_y);
parse_u32_field!(parse_popup_message_width, popup_message_width);
parse_u32_field!(parse_popup_message_height, popup_message_height);
parse_string_field!(parse_background_image, background_image);
parse_string_field!(parse_button_layout, button_layout);
parse_string_field!(parse_font_name, font_name);
parse_u32_field!(parse_font_size, font_size);

fn parse_text_color(_ini: &mut INI, scheme: &mut ControlBarScheme, args: &[&str]) -> INIResult<()> {
    let (r, g, b, a) = parse_rgba_tokens(args)?;
    scheme.set_text_color(r, g, b, a);
    Ok(())
}

fn parse_border_color(
    _ini: &mut INI,
    scheme: &mut ControlBarScheme,
    args: &[&str],
) -> INIResult<()> {
    let (r, g, b, a) = parse_rgba_tokens(args)?;
    scheme.set_border_color(r, g, b, a);
    Ok(())
}

fn parse_highlight_color(
    _ini: &mut INI,
    scheme: &mut ControlBarScheme,
    args: &[&str],
) -> INIResult<()> {
    let (r, g, b, a) = parse_rgba_tokens(args)?;
    scheme.set_highlight_color(r, g, b, a);
    Ok(())
}

const CONTROL_BAR_SCHEME_FIELDS: &[FieldParse<ControlBarScheme>] = &[
    FieldParse {
        token: "ScreenWidth",
        parse: parse_screen_width,
    },
    FieldParse {
        token: "ScreenHeight",
        parse: parse_screen_height,
    },
    FieldParse {
        token: "SidePanelWidth",
        parse: parse_side_panel_width,
    },
    FieldParse {
        token: "SidePanelHeight",
        parse: parse_side_panel_height,
    },
    FieldParse {
        token: "ButtonPanelWidth",
        parse: parse_button_panel_width,
    },
    FieldParse {
        token: "ButtonPanelHeight",
        parse: parse_button_panel_height,
    },
    FieldParse {
        token: "CommandBarWidth",
        parse: parse_command_bar_width,
    },
    FieldParse {
        token: "CommandBarHeight",
        parse: parse_command_bar_height,
    },
    FieldParse {
        token: "ControlBarX",
        parse: parse_control_bar_x,
    },
    FieldParse {
        token: "ControlBarY",
        parse: parse_control_bar_y,
    },
    FieldParse {
        token: "ControlBarWidth",
        parse: parse_control_bar_width,
    },
    FieldParse {
        token: "ControlBarHeight",
        parse: parse_control_bar_height,
    },
    FieldParse {
        token: "MinimapX",
        parse: parse_minimap_x,
    },
    FieldParse {
        token: "MinimapY",
        parse: parse_minimap_y,
    },
    FieldParse {
        token: "MinimapWidth",
        parse: parse_minimap_width,
    },
    FieldParse {
        token: "MinimapHeight",
        parse: parse_minimap_height,
    },
    FieldParse {
        token: "PowerBarX",
        parse: parse_power_bar_x,
    },
    FieldParse {
        token: "PowerBarY",
        parse: parse_power_bar_y,
    },
    FieldParse {
        token: "PowerBarWidth",
        parse: parse_power_bar_width,
    },
    FieldParse {
        token: "PowerBarHeight",
        parse: parse_power_bar_height,
    },
    FieldParse {
        token: "MoneyDisplayX",
        parse: parse_money_display_x,
    },
    FieldParse {
        token: "MoneyDisplayY",
        parse: parse_money_display_y,
    },
    FieldParse {
        token: "MoneyDisplayWidth",
        parse: parse_money_display_width,
    },
    FieldParse {
        token: "MoneyDisplayHeight",
        parse: parse_money_display_height,
    },
    FieldParse {
        token: "GeneralExpBarX",
        parse: parse_general_exp_bar_x,
    },
    FieldParse {
        token: "GeneralExpBarY",
        parse: parse_general_exp_bar_y,
    },
    FieldParse {
        token: "GeneralExpBarWidth",
        parse: parse_general_exp_bar_width,
    },
    FieldParse {
        token: "GeneralExpBarHeight",
        parse: parse_general_exp_bar_height,
    },
    FieldParse {
        token: "ChatPanelX",
        parse: parse_chat_panel_x,
    },
    FieldParse {
        token: "ChatPanelY",
        parse: parse_chat_panel_y,
    },
    FieldParse {
        token: "ChatPanelWidth",
        parse: parse_chat_panel_width,
    },
    FieldParse {
        token: "ChatPanelHeight",
        parse: parse_chat_panel_height,
    },
    FieldParse {
        token: "BeaconPanelX",
        parse: parse_beacon_panel_x,
    },
    FieldParse {
        token: "BeaconPanelY",
        parse: parse_beacon_panel_y,
    },
    FieldParse {
        token: "BeaconPanelWidth",
        parse: parse_beacon_panel_width,
    },
    FieldParse {
        token: "BeaconPanelHeight",
        parse: parse_beacon_panel_height,
    },
    FieldParse {
        token: "PopupMessageX",
        parse: parse_popup_message_x,
    },
    FieldParse {
        token: "PopupMessageY",
        parse: parse_popup_message_y,
    },
    FieldParse {
        token: "PopupMessageWidth",
        parse: parse_popup_message_width,
    },
    FieldParse {
        token: "PopupMessageHeight",
        parse: parse_popup_message_height,
    },
    FieldParse {
        token: "BackgroundImage",
        parse: parse_background_image,
    },
    FieldParse {
        token: "ButtonLayout",
        parse: parse_button_layout,
    },
    FieldParse {
        token: "FontName",
        parse: parse_font_name,
    },
    FieldParse {
        token: "FontSize",
        parse: parse_font_size,
    },
    FieldParse {
        token: "TextColor",
        parse: parse_text_color,
    },
    FieldParse {
        token: "BorderColor",
        parse: parse_border_color,
    },
    FieldParse {
        token: "HighlightColor",
        parse: parse_highlight_color,
    },
];

impl Default for ControlBarSchemeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global control bar scheme manager instance (thread-safe)
static CONTROL_BAR_SCHEME_MANAGER: OnceCell<Arc<RwLock<ControlBarSchemeManager>>> = OnceCell::new();

/// Ensure the control bar scheme manager exists and return a handle to it
pub fn ensure_control_bar_scheme_manager() -> Arc<RwLock<ControlBarSchemeManager>> {
    CONTROL_BAR_SCHEME_MANAGER
        .get_or_init(|| Arc::new(RwLock::new(ControlBarSchemeManager::new())))
        .clone()
}

/// Initialize (or reinitialize) the global control bar scheme manager
pub fn initialize_control_bar_scheme_manager() {
    let manager = ensure_control_bar_scheme_manager();
    manager.write().clear();
    load_control_bar_scheme_files();
}

fn load_control_bar_scheme_files() {
    let mut ini = INI::new();
    for path in [
        "Data/INI/Default/ControlBarScheme.ini",
        "Data/INI/ControlBarScheme.ini",
    ] {
        if let Err(err) = ini.load(path, INILoadType::Overwrite) {
            warn!("Failed to load control bar scheme INI '{}': {}", path, err);
        }
    }
}

/// Get the control bar scheme manager if it has been initialized
pub fn get_control_bar_scheme_manager() -> Option<Arc<RwLock<ControlBarSchemeManager>>> {
    CONTROL_BAR_SCHEME_MANAGER.get().cloned()
}

/// Parse control bar scheme definition from INI file
/// This is the main entry point called by the INI parser
pub fn parse_control_bar_scheme_definition(ini: &mut INI) -> INIResult<()> {
    // Read the scheme name
    let name = match ini.get_next_value_token().or_else(|| ini.get_first_token()) {
        Some(token) => token,
        None => return Err(INIError::InvalidData),
    };
    if name.trim().is_empty() {
        return Err(INIError::InvalidData);
    }

    let manager_handle = ensure_control_bar_scheme_manager();
    let mut manager = manager_handle.write();

    // Create new control bar scheme (or clear existing one)
    let scheme = manager.new_control_bar_scheme(name);

    // Parse the scheme fields using the field parsing table
    scheme.parse_scheme_fields(ini)?;

    // Validate the scheme
    scheme.validate()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_bar_scheme_creation() {
        let scheme = ControlBarScheme::new("TestScheme".to_string());
        assert_eq!(scheme.get_name(), "testscheme");
        assert_eq!(scheme.screen_width, 1024);
        assert_eq!(scheme.screen_height, 768);
    }

    #[test]
    fn test_control_bar_scheme_resolution_support() {
        let scheme = ControlBarScheme::new("TestScheme".to_string());
        assert!(scheme.supports_resolution(1024, 768));
        assert!(!scheme.supports_resolution(1280, 720));
    }

    #[test]
    fn test_control_bar_scheme_aspect_ratio() {
        let scheme = ControlBarScheme::new("TestScheme".to_string());
        let aspect_ratio = scheme.get_aspect_ratio();
        assert_eq!(aspect_ratio, 1024.0 / 768.0);
    }

    #[test]
    fn test_control_bar_scheme_area_checks() {
        let scheme = ControlBarScheme::new("TestScheme".to_string());

        // Test control bar area (default: x=0, y=568, w=1024, h=200)
        assert!(scheme.is_in_control_bar(100, 600));
        assert!(scheme.is_in_control_bar(0, 568));
        assert!(!scheme.is_in_control_bar(-1, 600));
        assert!(!scheme.is_in_control_bar(100, 567));

        // Test minimap area (default: x=10, y=578, w=160, h=120)
        assert!(scheme.is_in_minimap(50, 600));
        assert!(scheme.is_in_minimap(10, 578));
        assert!(!scheme.is_in_minimap(5, 600));
        assert!(!scheme.is_in_minimap(50, 577));
    }

    #[test]
    fn test_control_bar_scheme_colors() {
        let mut scheme = ControlBarScheme::new("TestScheme".to_string());

        // Test default colors
        let (r, g, b, a) = scheme.get_text_color();
        assert_eq!((r, g, b, a), (1.0, 1.0, 1.0, 1.0));

        // Test setting colors
        scheme.set_text_color(0.5, 0.6, 0.7, 0.8);
        let (r, g, b, a) = scheme.get_text_color();
        assert_eq!((r, g, b, a), (0.5, 0.6, 0.7, 0.8));
    }

    #[test]
    fn test_control_bar_scheme_validation_valid() {
        let scheme = ControlBarScheme::new("TestScheme".to_string());
        assert!(scheme.validate().is_ok());
    }

    #[test]
    fn test_control_bar_scheme_validation_invalid_dimensions() {
        let mut scheme = ControlBarScheme::new("TestScheme".to_string());
        scheme.screen_width = 0;
        assert!(scheme.validate().is_err());
    }

    #[test]
    fn test_control_bar_scheme_validation_invalid_colors() {
        let mut scheme = ControlBarScheme::new("TestScheme".to_string());
        scheme.text_color_r = 2.0; // Invalid color value
        assert!(scheme.validate().is_err());
    }

    #[test]
    fn test_control_bar_scheme_manager() {
        let mut manager = ControlBarSchemeManager::new();
        assert_eq!(manager.count(), 0);

        // Add a scheme
        let scheme = manager.new_control_bar_scheme("TestScheme".to_string());
        scheme.screen_width = 1280;

        assert_eq!(manager.count(), 1);

        // Find the scheme
        let found = manager.find_scheme("TestScheme");
        assert!(found.is_some());
        assert_eq!(found.unwrap().screen_width, 1280);

        // Set active scheme
        assert!(manager.set_active_scheme("TestScheme".to_string()).is_ok());
        let active = manager.get_active_scheme();
        assert!(active.is_some());
        assert_eq!(active.unwrap().screen_width, 1280);
    }

    #[test]
    fn test_control_bar_scheme_manager_resolution_matching() {
        let mut manager = ControlBarSchemeManager::new();

        // Add schemes for different resolutions
        let scheme1 = manager.new_control_bar_scheme("1024x768".to_string());
        scheme1.screen_width = 1024;
        scheme1.screen_height = 768;

        let scheme2 = manager.new_control_bar_scheme("1280x720".to_string());
        scheme2.screen_width = 1280;
        scheme2.screen_height = 720;

        // Test exact match
        let found = manager.find_scheme_for_resolution(1024, 768);
        assert!(found.is_some());
        assert_eq!(found.unwrap().get_name(), "1024x768");

        // Test no exact match (should find closest aspect ratio)
        let found = manager.find_scheme_for_resolution(1920, 1080);
        assert!(found.is_some());
        // 1920/1080 = 1.77, 1280/720 = 1.77, 1024/768 = 1.33
        // So 1280x720 should be closer
        assert_eq!(found.unwrap().get_name(), "1280x720");
    }
}
