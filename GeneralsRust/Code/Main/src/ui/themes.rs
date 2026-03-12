//! UI Themes and Styling
//!
//! This module defines the visual theme for Command & Conquer Generals UI.

use super::colors;

/// UI theme configuration
pub trait UITheme {
    fn get_background_color(&self) -> (u8, u8, u8);
    fn get_button_color(&self) -> (u8, u8, u8);
    fn get_button_hover_color(&self) -> (u8, u8, u8);
    fn get_text_color(&self) -> (u8, u8, u8);
    fn get_accent_color(&self) -> (u8, u8, u8);
}

/// Command & Conquer Generals theme
pub struct GeneralsTheme;

impl UITheme for GeneralsTheme {
    fn get_background_color(&self) -> (u8, u8, u8) {
        colors::BLUE_DARK
    }

    fn get_button_color(&self) -> (u8, u8, u8) {
        colors::BLUE_LIGHT
    }

    fn get_button_hover_color(&self) -> (u8, u8, u8) {
        colors::ORANGE
    }

    fn get_text_color(&self) -> (u8, u8, u8) {
        colors::WHITE
    }

    fn get_accent_color(&self) -> (u8, u8, u8) {
        colors::YELLOW
    }
}

/// Color constants
pub struct Colors;

impl Colors {
    pub const BLUE_DARK: (u8, u8, u8) = colors::BLUE_DARK;
    pub const BLUE_LIGHT: (u8, u8, u8) = colors::BLUE_LIGHT;
    pub const ORANGE: (u8, u8, u8) = colors::ORANGE;
    pub const WHITE: (u8, u8, u8) = colors::WHITE;
    pub const YELLOW: (u8, u8, u8) = colors::YELLOW;
}
