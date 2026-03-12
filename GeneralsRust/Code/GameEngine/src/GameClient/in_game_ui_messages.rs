// FILE: in_game_ui_messages.rs
// In-game UI message display system
// Ported from C++ to Rust

use std::collections::VecDeque;

/// Maximum number of UI messages to display
pub const MAX_UI_MESSAGES: usize = 6;

/// RGB Color structure
#[derive(Clone, Copy, Debug)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn as_int(&self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    pub fn with_alpha(&self, a: u8) -> u32 {
        ((a as u32) << 24) | self.as_int()
    }
}

/// RGBA Color as integer
pub type Color = u32;

pub fn make_color(r: u8, g: u8, b: u8, a: u8) -> Color {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Display string placeholder (would be replaced with actual rendering system)
#[derive(Clone, Debug)]
pub struct DisplayString {
    pub text: String,
    pub font_name: String,
    pub point_size: i32,
    pub is_bold: bool,
}

impl DisplayString {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            font_name: String::from("Arial"),
            point_size: 12,
            is_bold: false,
        }
    }

    pub fn set_font(&mut self, font_name: String, point_size: i32, is_bold: bool) {
        self.font_name = font_name;
        self.point_size = point_size;
        self.is_bold = is_bold;
    }

    pub fn set_text(&mut self, text: String) {
        self.text = text;
    }

    pub fn draw(&self, x: i32, y: i32, color: Color) {
        // Placeholder for actual rendering
        // In a real implementation, this would call into a rendering system
        println!("Drawing '{}' at ({}, {}) with color {:08X}", self.text, x, y, color);
    }
}

impl Default for DisplayString {
    fn default() -> Self {
        Self::new()
    }
}

/// A UI message
#[derive(Clone)]
pub struct UiMessage {
    pub full_text: String,
    pub display_string: Option<DisplayString>,
    pub timestamp: u32,
    pub color: Color,
}

impl UiMessage {
    pub fn new() -> Self {
        Self {
            full_text: String::new(),
            display_string: None,
            timestamp: 0,
            color: make_color(255, 255, 255, 255),
        }
    }

    pub fn is_expired(&self, current_frame: u32, message_delay_frames: u32) -> bool {
        if self.timestamp == 0 {
            return true;
        }
        current_frame > self.timestamp + message_delay_frames
    }
}

impl Default for UiMessage {
    fn default() -> Self {
        Self::new()
    }
}

/// In-game UI message manager
pub struct InGameUiMessages {
    messages: Vec<UiMessage>,
    messages_enabled: bool,
    message_color1: Color,
    message_color2: Color,
    message_position: (i32, i32),
    message_font: String,
    message_point_size: i32,
    message_bold: bool,
    message_delay_ms: i32,
    alternate_color: bool,
}

impl InGameUiMessages {
    pub fn new() -> Self {
        let mut messages = Vec::with_capacity(MAX_UI_MESSAGES);
        for _ in 0..MAX_UI_MESSAGES {
            messages.push(UiMessage::new());
        }

        Self {
            messages,
            messages_enabled: true,
            message_color1: make_color(255, 255, 255, 255), // White
            message_color2: make_color(255, 255, 0, 255),   // Yellow
            message_position: (10, 10),
            message_font: String::from("Arial"),
            message_point_size: 12,
            message_bold: false,
            message_delay_ms: 10000, // 10 seconds
            alternate_color: false,
        }
    }

    pub fn init(&mut self) {
        // Initialize
    }

    pub fn reset(&mut self) {
        for msg in &mut self.messages {
            msg.full_text.clear();
            msg.display_string = None;
            msg.timestamp = 0;
        }
    }

    pub fn toggle_messages(&mut self) {
        self.messages_enabled = !self.messages_enabled;
    }

    pub fn are_messages_enabled(&self) -> bool {
        self.messages_enabled
    }

    /// Display a message to the user
    pub fn message(&mut self, text: String, current_frame: u32) {
        self.add_message_text(text, None, current_frame);
    }

    /// Display a colored message to the user
    pub fn message_color(&mut self, rgb_color: &RgbColor, text: String, current_frame: u32) {
        self.add_message_text(text, Some(rgb_color), current_frame);
    }

    /// Add message text to the UI
    fn add_message_text(&mut self, formatted_message: String, rgb_color: Option<&RgbColor>, current_frame: u32) {
        let mut color1 = self.message_color1;
        let mut color2 = self.message_color2;

        if let Some(rgb) = rgb_color {
            color1 = rgb.with_alpha(255);
            color2 = rgb.with_alpha(255);
        }

        // Shift all messages down one index
        for i in (1..MAX_UI_MESSAGES).rev() {
            self.messages[i] = self.messages[i - 1].clone();
        }

        // Set the new message at index 0
        self.messages[0].full_text = formatted_message.clone();
        self.messages[0].timestamp = current_frame;

        // Alternate between colors
        let color = if self.alternate_color { color2 } else { color1 };
        self.alternate_color = !self.alternate_color;
        self.messages[0].color = color;

        // Create display string
        let mut display_string = DisplayString::new();
        display_string.set_font(
            self.message_font.clone(),
            self.message_point_size,
            self.message_bold,
        );
        display_string.set_text(formatted_message);

        self.messages[0].display_string = Some(display_string);
    }

    /// Draw all messages
    pub fn draw(&self, current_frame: u32) {
        if !self.messages_enabled {
            return;
        }

        let message_delay_frames = (self.message_delay_ms as f32 / 1000.0 * 30.0) as u32; // Assuming 30 FPS
        let mut y_offset = self.message_position.1;
        let line_height = self.message_point_size + 2;

        for msg in &self.messages {
            if msg.timestamp == 0 {
                continue;
            }

            // Check if message is expired
            if msg.is_expired(current_frame, message_delay_frames) {
                continue;
            }

            // Draw the message
            if let Some(ref display_string) = msg.display_string {
                display_string.draw(self.message_position.0, y_offset, msg.color);
                y_offset += line_height;
            }
        }
    }

    /// Remove message at index
    fn remove_message_at_index(&mut self, index: usize) {
        if index < MAX_UI_MESSAGES {
            self.messages[index].full_text.clear();
            self.messages[index].display_string = None;
            self.messages[index].timestamp = 0;
        }
    }

    /// Free message resources
    pub fn free_message_resources(&mut self) {
        for msg in &mut self.messages {
            msg.display_string = None;
        }
    }

    /// Set message colors
    pub fn set_message_colors(&mut self, color1: Color, color2: Color) {
        self.message_color1 = color1;
        self.message_color2 = color2;
    }

    /// Set message position
    pub fn set_message_position(&mut self, x: i32, y: i32) {
        self.message_position = (x, y);
    }

    /// Set message font
    pub fn set_message_font(&mut self, font_name: String, point_size: i32, bold: bool) {
        self.message_font = font_name;
        self.message_point_size = point_size;
        self.message_bold = bold;
    }

    /// Set message delay
    pub fn set_message_delay(&mut self, delay_ms: i32) {
        self.message_delay_ms = delay_ms;
    }

    pub fn get_message_color(&self, alt_color: bool) -> Color {
        if alt_color {
            self.message_color2
        } else {
            self.message_color1
        }
    }
}

impl Default for InGameUiMessages {
    fn default() -> Self {
        Self::new()
    }
}

/// Military subtitle data
pub const MAX_SUBTITLE_LINES: usize = 4;

#[derive(Clone)]
pub struct MilitarySubtitleData {
    pub subtitle: String,
    pub index: usize,
    pub position: (i32, i32),
    pub display_strings: Vec<Option<DisplayString>>,
    pub current_display_string: usize,
    pub lifetime: u32,
    pub block_drawn: bool,
    pub block_begin_frame: u32,
    pub block_pos: (i32, i32),
    pub increment_on_frame: u32,
    pub color: Color,
}

impl MilitarySubtitleData {
    pub fn new(
        subtitle: String,
        position: (i32, i32),
        lifetime: u32,
        color: Color,
    ) -> Self {
        let mut display_strings = Vec::with_capacity(MAX_SUBTITLE_LINES);
        for _ in 0..MAX_SUBTITLE_LINES {
            display_strings.push(None);
        }

        Self {
            subtitle,
            index: 0,
            position,
            display_strings,
            current_display_string: 0,
            lifetime,
            block_drawn: false,
            block_begin_frame: 0,
            block_pos: position,
            increment_on_frame: 0,
            color,
        }
    }
}

/// Military subtitle manager
pub struct MilitarySubtitleManager {
    current_subtitle: Option<MilitarySubtitleData>,
    caption_color: Color,
    caption_position: (i32, i32),
    title_font: String,
    title_point_size: i32,
    title_bold: bool,
    caption_font: String,
    caption_point_size: i32,
    caption_bold: bool,
    randomize_typing: bool,
    typing_speed: i32,
}

impl MilitarySubtitleManager {
    pub fn new() -> Self {
        Self {
            current_subtitle: None,
            caption_color: make_color(255, 255, 255, 255),
            caption_position: (10, 500),
            title_font: String::from("Arial"),
            title_point_size: 16,
            title_bold: true,
            caption_font: String::from("Arial"),
            caption_point_size: 14,
            caption_bold: false,
            randomize_typing: false,
            typing_speed: 50,
        }
    }

    pub fn add_subtitle(&mut self, text: String, duration_ms: i32, current_frame: u32) {
        let lifetime = (duration_ms as f32 / 1000.0 * 30.0) as u32; // Assuming 30 FPS
        self.current_subtitle = Some(MilitarySubtitleData::new(
            text,
            self.caption_position,
            lifetime,
            self.caption_color,
        ));

        if let Some(ref mut subtitle) = self.current_subtitle {
            subtitle.block_begin_frame = current_frame;
            subtitle.increment_on_frame = current_frame;
        }
    }

    pub fn remove_subtitle(&mut self) {
        self.current_subtitle = None;
    }

    pub fn update(&mut self, current_frame: u32) {
        if let Some(ref subtitle) = self.current_subtitle {
            if current_frame > subtitle.block_begin_frame + subtitle.lifetime {
                self.current_subtitle = None;
            }
        }
    }

    pub fn draw(&self) {
        if let Some(ref subtitle) = self.current_subtitle {
            // Split subtitle into lines
            let lines: Vec<&str> = subtitle.subtitle.split('\n').collect();

            let mut y_offset = subtitle.position.1;
            for (i, line) in lines.iter().enumerate().take(MAX_SUBTITLE_LINES) {
                if i < subtitle.display_strings.len() {
                    if let Some(ref display_string) = subtitle.display_strings[i] {
                        display_string.draw(subtitle.position.0, y_offset, subtitle.color);
                        y_offset += self.caption_point_size + 2;
                    }
                }
            }
        }
    }

    pub fn set_font(&mut self, title_font: String, title_size: i32, title_bold: bool,
                     caption_font: String, caption_size: i32, caption_bold: bool) {
        self.title_font = title_font;
        self.title_point_size = title_size;
        self.title_bold = title_bold;
        self.caption_font = caption_font;
        self.caption_point_size = caption_size;
        self.caption_bold = caption_bold;
    }
}

impl Default for MilitarySubtitleManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Floating text data
#[derive(Clone)]
pub struct FloatingTextData {
    pub color: Color,
    pub text: String,
    pub display_string: Option<DisplayString>,
    pub pos_3d: (f32, f32, f32),
    pub frame_timeout: u32,
    pub frame_count: u32,
}

pub const DEFAULT_FLOATING_TEXT_TIMEOUT: u32 = 10; // frames

impl FloatingTextData {
    pub fn new(text: String, pos: (f32, f32, f32), color: Color, timeout: u32) -> Self {
        Self {
            color,
            text,
            display_string: None,
            pos_3d: pos,
            frame_timeout: timeout,
            frame_count: 0,
        }
    }
}

/// Floating text manager
pub struct FloatingTextManager {
    texts: VecDeque<FloatingTextData>,
    timeout: u32,
    move_up_speed: f32,
    vanish_rate: f32,
}

impl FloatingTextManager {
    pub fn new() -> Self {
        Self {
            texts: VecDeque::new(),
            timeout: DEFAULT_FLOATING_TEXT_TIMEOUT,
            move_up_speed: 0.5,
            vanish_rate: 0.05,
        }
    }

    pub fn add_floating_text(&mut self, text: String, pos: (f32, f32, f32), color: Color) {
        let mut floating_text = FloatingTextData::new(text.clone(), pos, color, self.timeout);

        let mut display_string = DisplayString::new();
        display_string.set_text(text);
        floating_text.display_string = Some(display_string);

        self.texts.push_back(floating_text);
    }

    pub fn update(&mut self) {
        // Remove expired texts
        self.texts.retain(|text| text.frame_count < text.frame_timeout);

        // Update positions and frame counts
        for text in &mut self.texts {
            text.frame_count += 1;
            // Move text upward
            text.pos_3d.2 += self.move_up_speed;
        }
    }

    pub fn draw(&self) {
        for text in &self.texts {
            if let Some(ref display_string) = text.display_string {
                // Convert 3D position to 2D screen coordinates (simplified)
                let screen_x = text.pos_3d.0 as i32;
                let screen_y = text.pos_3d.1 as i32;

                // Calculate fade based on lifetime
                let fade = 1.0 - (text.frame_count as f32 / text.frame_timeout as f32);
                let alpha = (fade * 255.0) as u8;
                let faded_color = (text.color & 0x00FFFFFF) | ((alpha as u32) << 24);

                display_string.draw(screen_x, screen_y, faded_color);
            }
        }
    }

    pub fn clear(&mut self) {
        self.texts.clear();
    }
}

impl Default for FloatingTextManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let mut ui_messages = InGameUiMessages::new();
        ui_messages.message("Test message".to_string(), 100);

        assert_eq!(ui_messages.messages[0].full_text, "Test message");
        assert_eq!(ui_messages.messages[0].timestamp, 100);
    }

    #[test]
    fn test_message_expiration() {
        let msg = UiMessage {
            full_text: "Test".to_string(),
            display_string: None,
            timestamp: 100,
            color: make_color(255, 255, 255, 255),
        };

        assert!(!msg.is_expired(150, 100)); // Not expired
        assert!(msg.is_expired(250, 100));  // Expired
    }

    #[test]
    fn test_message_shift() {
        let mut ui_messages = InGameUiMessages::new();

        ui_messages.message("Message 1".to_string(), 100);
        ui_messages.message("Message 2".to_string(), 200);

        assert_eq!(ui_messages.messages[0].full_text, "Message 2");
        assert_eq!(ui_messages.messages[1].full_text, "Message 1");
    }

    #[test]
    fn test_color_creation() {
        let color = make_color(255, 128, 64, 32);
        assert_eq!(color, 0x20FF8040);
    }

    #[test]
    fn test_rgb_color() {
        let rgb = RgbColor::new(255, 128, 64);
        assert_eq!(rgb.as_int(), 0xFF8040);
        assert_eq!(rgb.with_alpha(32), 0x20FF8040);
    }

    #[test]
    fn test_floating_text() {
        let mut manager = FloatingTextManager::new();
        manager.add_floating_text("Test".to_string(), (100.0, 200.0, 0.0), make_color(255, 255, 255, 255));

        assert_eq!(manager.texts.len(), 1);

        manager.update();
        assert!(manager.texts[0].pos_3d.2 > 0.0); // Should have moved up
    }
}
