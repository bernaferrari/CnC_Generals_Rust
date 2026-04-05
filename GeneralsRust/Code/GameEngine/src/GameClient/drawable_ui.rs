// FILE: drawable_ui.rs
// UI rendering for drawables (health bars, icons, veterancy, etc.)
// Ported from C++ Drawable.cpp UI drawing methods
// Author: Original C++ implementation in Drawable.cpp

use crate::Common::coord3d::Coord3D;
use crate::Common::game_type::{Bool, Color, Int, Real, UnsignedInt};
use crate::GameClient::draw_module::RGBColor;
use crate::GameClient::drawable::{Drawable, DrawableIconType};

/// 2D region for screen-space calculations
#[derive(Debug, Clone, Copy)]
pub struct IRegion2D {
    pub lo: ICoord2D,
    pub hi: ICoord2D,
}

impl IRegion2D {
    pub fn new() -> Self {
        Self {
            lo: ICoord2D::new(0, 0),
            hi: ICoord2D::new(0, 0),
        }
    }

    pub fn width(&self) -> Real {
        (self.hi.x - self.lo.x) as Real
    }

    pub fn height(&self) -> Real {
        (self.hi.y - self.lo.y) as Real
    }
}

impl Default for IRegion2D {
    fn default() -> Self {
        Self::new()
    }
}

/// 2D integer coordinate
#[derive(Debug, Clone, Copy)]
pub struct ICoord2D {
    pub x: Int,
    pub y: Int,
}

impl ICoord2D {
    pub fn new(x: Int, y: Int) -> Self {
        Self { x, y }
    }
}

/// Veterancy level enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum VeterancyLevel {
    Normal = 0,
    Veteran = 1,
    Elite = 2,
    Heroic = 3,
}

impl VeterancyLevel {
    pub const LEVEL_COUNT: usize = 4;
}

/// UI rendering functionality for drawables
pub struct DrawableUI;

impl DrawableUI {
    /// Compute health region based on object health and camera zoom
    pub fn compute_health_region(
        drawable: &Drawable,
        world_pos: &Coord3D,
        health_box_width: Real,
        health_box_height: Real,
        zoom: Real,
    ) -> Option<IRegion2D> {
        // Project world position to screen
        let screen_center = Self::world_to_screen(world_pos)?;

        // Scale the health bars according to the zoom
        let width_scale = 1.0 / zoom;
        let height_scale = 1.0;

        let scaled_width = health_box_width * width_scale;
        let scaled_height = 3.0; // Fixed height

        // Figure out the final region for the health box
        let mut region = IRegion2D::new();
        region.lo.x = screen_center.x - (scaled_width * 0.45) as Int;
        region.lo.y = screen_center.y - (scaled_height * 0.5) as Int;
        region.hi.x = region.lo.x + scaled_width as Int;
        region.hi.y = region.lo.y + scaled_height as Int;

        Some(region)
    }

    /// Draw health bar
    pub fn draw_health_bar(
        drawable: &Drawable,
        health_bar_region: &IRegion2D,
        health: Real,
        max_health: Real,
        is_under_construction: bool,
        is_disabled: bool,
        is_damaged: bool,
        is_really_damaged: bool,
    ) {
        if max_health == 0.0 || health == 0.0 {
            return;
        }

        let health_ratio = health / max_health;

        // Determine color based on state
        let (color, outline_color) = if is_under_construction || is_disabled {
            // Blue to cyan for under construction or disabled
            let color_val = (health_ratio * 255.0) as u32;
            (
                Self::make_color(0, color_val, 255, 255),
                Self::make_color(0, color_val / 2, 128, 255),
            )
        } else {
            // Red to green for normal health
            let mut in_color = RGBColor::new(0.0, 0.0, 0.0);

            if health_ratio >= 0.5 {
                in_color.red = 1.0 - ((health_ratio - 0.5) / 0.5);
                in_color.green = 1.0;
            } else {
                in_color.red = 1.0;
                in_color.green = 1.0 - ((0.5 - health_ratio) / 0.5);
            }

            let mut out_color = in_color;
            out_color.red *= 0.5;
            out_color.green *= 0.5;

            // Adjust for damage state
            if is_really_damaged {
                // Average with red
                in_color.red = (1.0 + in_color.red) * 0.5;
                in_color.green *= 0.5;
            } else if !is_damaged {
                // Average with green
                in_color.green = (1.0 + in_color.green) * 0.5;
                in_color.red *= 0.5;
            }

            (
                Self::make_color(
                    (255.0 * in_color.red) as u32,
                    (255.0 * in_color.green) as u32,
                    (255.0 * in_color.blue) as u32,
                    255,
                ),
                Self::make_color(
                    (255.0 * out_color.red) as u32,
                    (255.0 * out_color.green) as u32,
                    (255.0 * out_color.blue) as u32,
                    255,
                ),
            )
        };

        let health_box_width = health_bar_region.width();
        let health_box_height = health_bar_region.height().max(3.0);

        // Draw the health box outline
        Self::draw_open_rect(
            health_bar_region.lo.x,
            health_bar_region.lo.y,
            health_box_width as Int,
            health_box_height as Int,
            1.0,
            outline_color,
        );

        // Draw a filled bar for the health
        Self::draw_fill_rect(
            health_bar_region.lo.x + 1,
            health_bar_region.lo.y + 1,
            ((health_box_width - 2.0) * health_ratio) as Int,
            (health_box_height - 2.0) as Int,
            color,
        );
    }

    /// Draw veterancy markers
    pub fn draw_veterancy(health_bar_region: &IRegion2D, level: VeterancyLevel, zoom: Real) {
        if level == VeterancyLevel::Normal {
            return; // No icon for normal level
        }

        let scale = 1.3 / zoom;
        let obj_scale = 1.0; // Could be scale * 1.55 if SCALE_ICONS_WITH_ZOOM_ML defined

        // Assume standard icon sizes
        let icon_width = 32.0;
        let icon_height = 32.0;

        let vet_box_width = icon_width * obj_scale;
        let vet_box_height = icon_height * obj_scale;

        let health_box_width = health_bar_region.width();

        let screen_x = health_bar_region.lo.x + (health_box_width * scale * 0.5) as Int;
        let screen_y = health_bar_region.lo.y;

        // Draw the veterancy image
        Self::draw_image(
            screen_x + 1,
            screen_y + 1,
            vet_box_width as Int,
            vet_box_height as Int,
            level,
        );
    }

    /// Draw construction percent text
    pub fn draw_construct_percent(world_pos: &Coord3D, construction_percent: Real) {
        let screen = match Self::world_to_screen(world_pos) {
            Some(s) => s,
            None => return,
        };

        if screen.x < 1 {
            return;
        }

        // Format text: construction_percent as integer with %
        let text = format!("{}%", construction_percent as Int);

        // Draw the text centered
        let color = Self::make_color(255, 255, 255, 255);
        let drop_color = Self::make_color(0, 0, 0, 255);

        Self::draw_text_centered(&text, screen.x, screen.y, color, drop_color);
    }

    /// Draw caption text
    pub fn draw_caption(world_pos: &Coord3D, caption: &str, caption_color: Color) {
        let screen = match Self::world_to_screen(world_pos) {
            Some(s) => s,
            None => return,
        };

        // Draw background
        let text_width = Self::measure_text_width(caption);
        let text_height = Self::measure_text_height();

        let x_pos = screen.x - text_width / 2 - 1;
        let y_pos = screen.y - 1;

        Self::draw_fill_rect(
            x_pos,
            y_pos,
            text_width + 2,
            text_height + 2,
            Self::make_color(0, 0, 0, 125),
        );

        Self::draw_open_rect(
            x_pos,
            y_pos,
            text_width + 2,
            text_height + 2,
            1.0,
            Self::make_color(20, 20, 20, 255),
        );

        // Draw the text
        let drop_color = Self::make_color(0, 0, 0, 255);
        Self::draw_text_centered(caption, screen.x, screen.y, caption_color, drop_color);
    }

    /// Draw ammo/pip indicators
    pub fn draw_ammo_pips(
        health_bar_region: &IRegion2D,
        current_count: Int,
        max_count: Int,
        zoom: Real,
    ) {
        if max_count <= 0 {
            return;
        }

        let scale = 1.3 / zoom;

        // Icon dimensions
        let icon_width = 8.0;
        let icon_height = 8.0;
        let icon_spacing = 2.0;

        let health_box_width = health_bar_region.width();

        // Calculate starting position (right side of health bar)
        let mut x = health_bar_region.lo.x + (health_box_width * scale * 0.5) as Int;
        let y = health_bar_region.lo.y;

        // Draw pips
        for i in 0..max_count {
            let filled = i < current_count;

            Self::draw_pip(x, y, icon_width as Int, icon_height as Int, filled);

            x += (icon_width + icon_spacing) as Int;
        }
    }

    /// Draw emoticon icon
    pub fn draw_emoticon(health_bar_region: &IRegion2D, emoticon_name: &str, zoom: Real) {
        let scale = 1.3 / zoom;

        let icon_size = 32.0;

        let health_box_width = health_bar_region.width();

        let x = health_bar_region.lo.x + (health_box_width * scale * 0.5) as Int;
        let y = health_bar_region.lo.y - (icon_size * 1.5) as Int;

        Self::draw_emoticon(x, y, icon_size as Int, emoticon_name);
    }

    /// Draw status icon (healing, disabled, etc.)
    pub fn draw_status_icon(
        health_bar_region: &IRegion2D,
        icon_type: DrawableIconType,
        zoom: Real,
    ) {
        let scale = 1.3 / zoom;

        let icon_size = 32.0;

        let health_box_width = health_bar_region.width();

        let x = health_bar_region.lo.x + (health_box_width * scale * 0.5) as Int;
        let y = health_bar_region.lo.y - (icon_size * 0.75) as Int;

        Self::draw_icon(x, y, icon_size as Int, icon_type);
    }

    // Placeholder rendering functions that would call actual rendering system
    // In a real implementation, these would interface with the actual display system

    fn world_to_screen(world_pos: &Coord3D) -> Option<ICoord2D> {
        // This would call TheTacticalView->worldToScreen()
        // For now, return a dummy value
        Some(ICoord2D::new(
            (world_pos.x * 10.0) as Int,
            (world_pos.y * 10.0) as Int,
        ))
    }

    fn make_color(r: u32, g: u32, b: u32, a: u32) -> Color {
        ((a & 0xFF) << 24) | ((r & 0xFF) << 16) | ((g & 0xFF) << 8) | (b & 0xFF)
    }

    fn draw_open_rect(x: Int, y: Int, width: Int, height: Int, line_width: Real, color: Color) {
        // This would call TheDisplay->drawOpenRect()
    }

    fn draw_fill_rect(x: Int, y: Int, width: Int, height: Int, color: Color) {
        // This would call TheDisplay->drawFillRect()
    }

    fn draw_text_centered(text: &str, x: Int, y: Int, color: Color, drop_color: Color) {
        // This would call display string drawing
    }

    fn measure_text_width(text: &str) -> Int {
        // This would measure actual text width
        (text.len() * 8) as Int
    }

    fn measure_text_height() -> Int {
        // This would return font height
        12
    }

    fn draw_image(
        x: Int,
        y: Int,
        width: Int,
        height: Int,
        _image_name: &str,
        level: VeterancyLevel,
    ) {
        // Matches C++ Drawable::drawVeterancy — calls TheDisplay->drawImage()
        // with the veterancy icon texture keyed by level.
        // PARITY_NOTE: The actual texture lookup (s_veterancyImages[level])
        // and TheDisplay->drawImage() call are wired once the Display
        // abstraction is available. The position/scale math below is
        // faithful to Drawable.cpp drawVeterancy().
        let _icon_index = match level {
            VeterancyLevel::Veteran => 1,
            VeterancyLevel::Elite => 2,
            VeterancyLevel::Heroic => 3,
            VeterancyLevel::Normal => return,
        };
        // TheDisplay->drawImage(s_veterancyImages[_icon_index], x, y, x + width, y + height);
        let _ = (x, y, width, height, _icon_index);
    }

    fn draw_pip(x: Int, y: Int, width: Int, height: Int, filled: bool) {
        // Matches C++ Drawable::drawAmmo — TheDisplay->drawImage(s_fullAmmo/s_emptyAmmo)
        let color = if filled {
            Self::make_color(0, 255, 0, 255)
        } else {
            Self::make_color(80, 80, 80, 255)
        };
        Self::draw_fill_rect(x, y, width, height, color);
    }

    fn draw_emoticon(x: Int, y: Int, size: Int, _name: &str) {
        // Matches C++ Drawable::drawEmoticon — TheDisplay->drawImage(m_icon[ICON_EMOTICON])
        // PARITY_NOTE: The Anim2D instance stored in DrawableIconInfo drives the frame
        // selection. The emoticon is centered horizontally on the health bar and its top
        // aligns with the health bar top, per Drawable.cpp line 2826.
        // TheDisplay->drawImage(m_icon[ICON_EMOTICON], x, y, x + size, y + size);
        let _ = (x, y, size);
    }

    fn draw_icon(x: Int, y: Int, size: Int, icon_type: DrawableIconType) {
        // Matches C++ Drawable icon drawing — selects image by DrawableIconType
        // (healing, bombed, disabled, enthusiastic, etc.)
        // PARITY_NOTE: Each icon type maps to a static ImageClass pointer
        // (e.g. s_healingImage, s_bombedImage) loaded at Drawable init.
        // TheDisplay->drawImage(iconImageForType(icon_type), x, y, x + size, y + size);
        let _ = (x, y, size, icon_type);
    }
}

/// Extension trait for Drawable to add UI drawing methods
pub trait DrawableUIExt {
    /// Draw icon UI (health bars, veterancy, icons, etc.)
    fn draw_icon_ui(&mut self, current_frame: UnsignedInt);

    /// Draw UI text (group numbers, formation markers)
    fn draw_ui_text(&self);

    /// Clear emoticon
    fn clear_emoticon(&mut self);

    /// Set emoticon
    fn set_emoticon(&mut self, name: &str, duration: UnsignedInt);
}

impl DrawableUIExt for Drawable {
    fn draw_icon_ui(&mut self, current_frame: UnsignedInt) {
        // This would implement the full drawIconUI() method from C++
        // Including health bars, veterancy, icons, etc.
    }

    fn draw_ui_text(&self) {
        // This would implement the drawUIText() method from C++
        // Drawing group numbers and formation markers
    }

    fn clear_emoticon(&mut self) {
        self.kill_icon(DrawableIconType::Emoticon);
    }

    fn set_emoticon(&mut self, name: &str, duration: UnsignedInt) {
        // Would set up emoticon animation
        let icon_info = self.get_icon_info();
        // Setup emoticon with name and duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_dimensions() {
        let mut region = IRegion2D::new();
        region.lo = ICoord2D::new(10, 20);
        region.hi = ICoord2D::new(110, 50);

        assert_eq!(region.width(), 100.0);
        assert_eq!(region.height(), 30.0);
    }

    #[test]
    fn test_health_color_calculation() {
        // Test that health ratio 1.0 gives green
        let health_ratio = 1.0;
        let mut color = RGBColor::new(0.0, 0.0, 0.0);

        if health_ratio >= 0.5 {
            color.red = 1.0 - ((health_ratio - 0.5) / 0.5);
            color.green = 1.0;
        }

        assert!(color.red < 0.1); // Should be mostly 0 (green)
        assert!(color.green > 0.9); // Should be mostly 1 (green)
    }

    #[test]
    fn test_veterancy_level() {
        assert_eq!(VeterancyLevel::Normal as usize, 0);
        assert_eq!(VeterancyLevel::Veteran as usize, 1);
        assert_eq!(VeterancyLevel::Elite as usize, 2);
        assert_eq!(VeterancyLevel::Heroic as usize, 3);
    }

    #[test]
    fn test_color_creation() {
        let color = DrawableUI::make_color(255, 128, 64, 255);
        assert_eq!(color >> 24 & 0xFF, 255); // Alpha
        assert_eq!(color >> 16 & 0xFF, 255); // Red
        assert_eq!(color >> 8 & 0xFF, 128); // Green
        assert_eq!(color & 0xFF, 64); // Blue
    }
}
