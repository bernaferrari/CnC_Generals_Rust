//! DrawGroupInfo defaults for group number rendering.

use std::sync::{OnceLock, RwLock};

use crate::color::{game_make_color, Color};

#[derive(Debug, Clone)]
pub struct DrawGroupInfo {
    pub font_name: String,
    pub font_size: i32,
    pub font_is_bold: bool,
    pub use_player_color: bool,
    pub color_for_text: Color,
    pub color_for_text_drop_shadow: Color,
    pub drop_shadow_offset_x: i32,
    pub drop_shadow_offset_y: i32,
    pub percent_offset_x: f32,
    pub using_pixel_offset_x: bool,
    pub pixel_offset_y: i32,
    pub using_pixel_offset_y: bool,
}

impl Default for DrawGroupInfo {
    fn default() -> Self {
        Self {
            font_name: "Arial".to_string(),
            font_size: 10,
            font_is_bold: false,
            use_player_color: true,
            color_for_text: game_make_color(255, 255, 255, 255),
            color_for_text_drop_shadow: game_make_color(0, 0, 0, 255),
            drop_shadow_offset_x: -1,
            drop_shadow_offset_y: -1,
            percent_offset_x: -0.05,
            using_pixel_offset_x: false,
            pixel_offset_y: -10,
            using_pixel_offset_y: true,
        }
    }
}

static DRAW_GROUP_INFO: OnceLock<RwLock<DrawGroupInfo>> = OnceLock::new();

pub fn get_draw_group_info() -> &'static RwLock<DrawGroupInfo> {
    DRAW_GROUP_INFO.get_or_init(|| RwLock::new(DrawGroupInfo::default()))
}
